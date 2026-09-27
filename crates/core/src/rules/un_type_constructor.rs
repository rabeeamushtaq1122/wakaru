use std::collections::HashSet;
use swc_core::common::{Mark, Span, Spanned, SyntaxContext, DUMMY_SP};

use swc_core::ecma::ast::{
    ArrayLit, ArrowExpr, AssignExpr, BinExpr, BinaryOp, CallExpr, Class, ClassExpr, Constructor,
    Expr, ExprOrSpread, FnExpr, Function, Ident, Lit, Module, Number, Str, UnaryExpr, UnaryOp,
};
use swc_core::ecma::utils::ExprFactory;
use swc_core::ecma::visit::{Visit, VisitMut, VisitMutWith, VisitWith};

use super::RewriteLevel;
use crate::rules::eval_utils::module_blocks_global_reference;

pub struct UnTypeConstructor {
    level: RewriteLevel,
    /// Context for the synthesized `String` / `Number` / `Boolean` references:
    /// they name the globals, so they carry the unresolved mark like any other
    /// generated global reference.
    unresolved_ctxt: SyntaxContext,
    blocked_scopes: Vec<HashSet<String>>,
}

impl UnTypeConstructor {
    pub fn new(unresolved_mark: Mark, level: RewriteLevel) -> Self {
        Self {
            level,
            unresolved_ctxt: SyntaxContext::empty().apply_mark(unresolved_mark),
            blocked_scopes: Vec::new(),
        }
    }

    fn blocked(&self, name: &str) -> bool {
        self.blocked_scopes.iter().any(|scope| scope.contains(name))
    }

    fn with_scope(&mut self, names: HashSet<String>, visit: impl FnOnce(&mut Self)) {
        self.blocked_scopes.push(names);
        visit(self);
        self.blocked_scopes.pop();
    }

    fn can_synthesize(&self, name: &str) -> bool {
        !self.blocked(name)
    }
}

impl VisitMut for UnTypeConstructor {
    fn visit_mut_module(&mut self, module: &mut Module) {
        if self.level < RewriteLevel::Aggressive {
            return;
        }
        // A module-wide dynamic scope can introduce any of the names at
        // runtime. Statically declared names are handled per lexical scope
        // below, so unrelated top-level recoveries remain usable.
        if ["Number", "String", "Array"]
            .iter()
            .any(|name| module_blocks_global_reference(module, name))
        {
            return;
        }
        if module_has_constructor_write(module) {
            return;
        }
        let names = module_scope_names(module);
        self.with_scope(names, |this| module.visit_mut_children_with(this));
    }

    fn visit_mut_function(&mut self, function: &mut Function) {
        let names = function_scope_names(function);
        self.with_scope(names, |this| function.visit_mut_children_with(this));
    }

    fn visit_mut_arrow_expr(&mut self, arrow: &mut ArrowExpr) {
        let names = arrow_scope_names(arrow);
        self.with_scope(names, |this| arrow.visit_mut_children_with(this));
    }

    fn visit_mut_fn_expr(&mut self, expression: &mut FnExpr) {
        let mut names = HashSet::new();
        if let Some(ident) = &expression.ident {
            names.insert(ident.sym.to_string());
        }
        self.with_scope(names, |this| expression.visit_mut_children_with(this));
    }

    fn visit_mut_class_expr(&mut self, expression: &mut ClassExpr) {
        let mut names = HashSet::new();
        if let Some(ident) = &expression.ident {
            names.insert(ident.sym.to_string());
        }
        self.with_scope(names, |this| expression.visit_mut_children_with(this));
    }

    fn visit_mut_constructor(&mut self, constructor: &mut Constructor) {
        let names = constructor_scope_names(constructor);
        self.with_scope(names, |this| constructor.visit_mut_children_with(this));
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if self.level < RewriteLevel::Aggressive {
            return;
        }
        expr.visit_mut_children_with(self);

        let original_span = expr.span();
        match expr {
            // +x → Number(x) — only when x is an Ident
            Expr::Unary(UnaryExpr {
                op: UnaryOp::Plus,
                arg,
                ..
            }) if matches!(**arg, Expr::Ident(_)) && self.can_synthesize("Number") => {
                let arg = std::mem::replace(
                    arg,
                    Box::new(Expr::Lit(Lit::Num(Number {
                        span: DUMMY_SP,
                        value: 0.0,
                        raw: None,
                    }))),
                );
                *expr = make_call("Number", arg, original_span, self.unresolved_ctxt);
            }

            // x + "" → String(x)  OR  "str" + "" → "str"
            Expr::Bin(BinExpr {
                op: BinaryOp::Add,
                left,
                right,
                ..
            }) if is_empty_string(right) => {
                if is_string_lit(left) {
                    let left = std::mem::replace(
                        left,
                        Box::new(Expr::Lit(Lit::Num(Number {
                            span: DUMMY_SP,
                            value: 0.0,
                            raw: None,
                        }))),
                    );
                    *expr = *left;
                } else if self.can_synthesize("String") {
                    let left = std::mem::replace(
                        left,
                        Box::new(Expr::Lit(Lit::Num(Number {
                            span: DUMMY_SP,
                            value: 0.0,
                            raw: None,
                        }))),
                    );
                    *expr = make_call("String", left, original_span, self.unresolved_ctxt);
                }
            }

            // [,,,] → Array(n) — all-holes array with n > 0
            Expr::Array(ArrayLit { elems, .. })
                if is_all_holes(elems) && !elems.is_empty() && self.can_synthesize("Array") =>
            {
                let n = elems.len();
                *expr = make_call(
                    "Array",
                    Box::new(Expr::Lit(Lit::Num(Number {
                        span: DUMMY_SP,
                        value: n as f64,
                        raw: None,
                    }))),
                    original_span,
                    self.unresolved_ctxt,
                );
            }

            _ => {}
        }
    }
}

fn make_call(name: &str, arg: Box<Expr>, span: Span, unresolved_ctxt: SyntaxContext) -> Expr {
    Expr::Call(CallExpr {
        span,
        ctxt: Default::default(),
        callee: Expr::Ident(Ident::new(name.into(), DUMMY_SP, unresolved_ctxt)).as_callee(),
        args: vec![arg.as_arg()],
        type_args: None,
    })
}

fn is_empty_string(expr: &Expr) -> bool {
    matches!(expr, Expr::Lit(Lit::Str(Str { value, .. })) if value.is_empty())
}

fn is_string_lit(expr: &Expr) -> bool {
    matches!(expr, Expr::Lit(Lit::Str(_)))
}

fn is_all_holes(elems: &[Option<ExprOrSpread>]) -> bool {
    elems.iter().all(|e| e.is_none())
}

struct ScopeNameCollector {
    names: HashSet<String>,
}

impl ScopeNameCollector {
    fn new() -> Self {
        Self {
            names: HashSet::new(),
        }
    }
}

impl Visit for ScopeNameCollector {
    fn visit_binding_ident(&mut self, ident: &swc_core::ecma::ast::BindingIdent) {
        self.names.insert(ident.id.sym.to_string());
    }

    fn visit_fn_decl(&mut self, declaration: &swc_core::ecma::ast::FnDecl) {
        self.names.insert(declaration.ident.sym.to_string());
    }

    fn visit_class_decl(&mut self, declaration: &swc_core::ecma::ast::ClassDecl) {
        self.names.insert(declaration.ident.sym.to_string());
    }

    fn visit_function(&mut self, _: &Function) {}

    fn visit_arrow_expr(&mut self, _: &ArrowExpr) {}

    fn visit_fn_expr(&mut self, _: &FnExpr) {}

    fn visit_class(&mut self, _: &Class) {}

    fn visit_class_expr(&mut self, _: &ClassExpr) {}

    fn visit_constructor(&mut self, _: &Constructor) {}
}

fn module_scope_names(module: &Module) -> HashSet<String> {
    let mut collector = ScopeNameCollector::new();
    module.visit_with(&mut collector);
    collector.names
}

fn function_scope_names(function: &Function) -> HashSet<String> {
    let mut collector = ScopeNameCollector::new();
    function.params.visit_with(&mut collector);
    function.body.visit_with(&mut collector);
    collector.names
}

fn arrow_scope_names(arrow: &ArrowExpr) -> HashSet<String> {
    let mut collector = ScopeNameCollector::new();
    arrow.params.visit_with(&mut collector);
    arrow.body.visit_with(&mut collector);
    collector.names
}

fn constructor_scope_names(constructor: &Constructor) -> HashSet<String> {
    let mut collector = ScopeNameCollector::new();
    constructor.params.visit_with(&mut collector);
    if let Some(body) = &constructor.body {
        body.visit_with(&mut collector);
    }
    collector.names
}

struct ConstructorWriteFinder {
    found: bool,
}

impl Visit for ConstructorWriteFinder {
    fn visit_assign_expr(&mut self, assignment: &AssignExpr) {
        if matches!(assignment.left.as_expr(), Expr::Ident(id) if matches!(id.sym.as_ref(), "Number" | "String" | "Array"))
        {
            self.found = true;
            return;
        }
        assignment.visit_children_with(self);
    }

    fn visit_update_expr(&mut self, update: &swc_core::ecma::ast::UpdateExpr) {
        if matches!(update.arg.as_expr(), Expr::Ident(id) if matches!(id.sym.as_ref(), "Number" | "String" | "Array"))
        {
            self.found = true;
            return;
        }
        update.visit_children_with(self);
    }

    fn visit_unary_expr(&mut self, unary: &swc_core::ecma::ast::UnaryExpr) {
        if unary.op == swc_core::ecma::ast::UnaryOp::Delete
            && matches!(unary.arg.as_expr(), Expr::Ident(id) if matches!(id.sym.as_ref(), "Number" | "String" | "Array"))
        {
            self.found = true;
            return;
        }
        unary.visit_children_with(self);
    }
}

fn module_has_constructor_write(module: &Module) -> bool {
    let mut finder = ConstructorWriteFinder { found: false };
    module.visit_with(&mut finder);
    finder.found
}

#[cfg(test)]
mod tests {
    use swc_core::common::{sync::Lrc, Globals, Mark, SourceMap, GLOBALS};
    use swc_core::ecma::ast::{Callee, Decl, ModuleItem, Stmt};
    use swc_core::ecma::transforms::base::resolver;

    use super::*;

    #[test]
    fn synthesized_number_callee_carries_the_unresolved_mark() {
        GLOBALS.set(&Globals::new(), || {
            let cm: Lrc<SourceMap> = Default::default();
            let mut module = crate::unpacker::parse_es_module("const a = +x;", "fixture.js", cm)
                .expect("fixture should parse");
            let unresolved_mark = Mark::new();
            module.visit_mut_with(&mut resolver(unresolved_mark, Mark::new(), false));

            module.visit_mut_with(&mut UnTypeConstructor::new(
                unresolved_mark,
                RewriteLevel::Aggressive,
            ));

            let ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) = &module.body[0] else {
                panic!("expected a var declaration");
            };
            let init = var.decls[0].init.as_deref().expect("init");
            let Expr::Call(call) = init else {
                panic!("expected Number(x), got {init:?}");
            };
            let Callee::Expr(callee) = &call.callee else {
                panic!("expected an expression callee");
            };
            let Expr::Ident(id) = callee.as_ref() else {
                panic!("expected an identifier callee");
            };
            assert_eq!(id.sym.as_ref(), "Number");
            assert_eq!(id.ctxt.outer(), unresolved_mark);
        });
    }
}
