use swc_core::common::{Mark, SyntaxContext};
use swc_core::ecma::ast::{
    ArrowExpr, BinExpr, BinaryOp, BlockStmt, CallExpr, Callee, CatchClause, ClassDecl, ClassExpr,
    Decl, Expr, FnDecl, FnExpr, Function, Ident, Lit, Module, Stmt, UnaryExpr, UnaryOp,
    VarDeclKind, WithStmt,
};
use swc_core::ecma::visit::{Visit, VisitMut, VisitMutWith, VisitWith};

use super::expr_utils::is_unresolved_ident;

pub struct UnInfinity {
    unresolved_mark: Mark,
    blocked: bool,
}

impl UnInfinity {
    pub fn new(unresolved_mark: Mark) -> Self {
        Self { unresolved_mark, blocked: false }
    }
}

impl VisitMut for UnInfinity {
    fn visit_mut_module(&mut self, module: &mut Module) {
        let old = self.blocked;
        self.blocked |= module_binds_infinity(module) || has_dynamic_scope(module, self.unresolved_mark);
        module.visit_mut_children_with(self);
        self.blocked = old;
    }

    fn visit_mut_function(&mut self, function: &mut Function) {
        let old = self.blocked;
        self.blocked |= function.params.iter().any(|p| pat_binds_infinity(&p.pat))
            || function.body.as_ref().is_some_and(|b| {
                body_binds_infinity(&b.stmts) || has_dynamic_scope(b, self.unresolved_mark)
            });
        function.visit_mut_children_with(self);
        self.blocked = old;
    }

    fn visit_mut_arrow_expr(&mut self, arrow: &mut ArrowExpr) {
        let old = self.blocked;
        self.blocked |= arrow.params.iter().any(pat_binds_infinity)
            || has_dynamic_scope(&arrow.body, self.unresolved_mark);
        arrow.visit_mut_children_with(self);
        self.blocked = old;
    }

    fn visit_mut_fn_expr(&mut self, expr: &mut FnExpr) {
        let old = self.blocked;
        self.blocked |= expr.ident.as_ref().is_some_and(|id| id.sym == "Infinity");
        expr.visit_mut_children_with(self);
        self.blocked = old;
    }

    fn visit_mut_class_expr(&mut self, expr: &mut ClassExpr) {
        let old = self.blocked;
        self.blocked |= expr.ident.as_ref().is_some_and(|id| id.sym == "Infinity");
        expr.visit_mut_children_with(self);
        self.blocked = old;
    }

    fn visit_mut_block_stmt(&mut self, block: &mut BlockStmt) {
        let old = self.blocked;
        self.blocked |= block.stmts.iter().any(|stmt| match stmt {
            Stmt::Decl(Decl::Var(var)) if var.kind != VarDeclKind::Var =>
                var.decls.iter().any(|d| pat_binds_infinity(&d.name)),
            Stmt::Decl(Decl::Fn(FnDecl { ident, .. }))
            | Stmt::Decl(Decl::Class(ClassDecl { ident, .. })) => ident.sym == "Infinity",
            _ => false,
        });
        block.visit_mut_children_with(self);
        self.blocked = old;
    }

    fn visit_mut_catch_clause(&mut self, catch: &mut CatchClause) {
        let old = self.blocked;
        self.blocked |= catch.param.as_ref().is_some_and(pat_binds_infinity);
        catch.visit_mut_children_with(self);
        self.blocked = old;
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        expr.visit_mut_children_with(self);

        if self.blocked {
            return;
        }

        if let Expr::Bin(BinExpr {
            op: BinaryOp::Div,
            left,
            right,
            span,
        }) = expr
        {
            if !matches!(&**right, Expr::Lit(Lit::Num(num)) if num.value == 0.0) {
                return;
            }

            if matches!(&**left, Expr::Lit(Lit::Num(num)) if num.value == 1.0) {
                *expr = Expr::Ident(Ident::new(
                    "Infinity".into(),
                    *span,
                    SyntaxContext::empty().apply_mark(self.unresolved_mark),
                ));
                return;
            }

            if matches!(&**left, Expr::Unary(UnaryExpr { op: UnaryOp::Minus, arg, .. }) if matches!(&**arg, Expr::Lit(Lit::Num(num)) if num.value == 1.0))
            {
                *expr = Expr::Unary(UnaryExpr {
                    span: *span,
                    op: UnaryOp::Minus,
                    arg: Box::new(Expr::Ident(Ident::new(
                        "Infinity".into(),
                        *span,
                        SyntaxContext::empty().apply_mark(self.unresolved_mark),
                    ))),
                });
            }
        }
    }
}

fn module_binds_infinity(module: &Module) -> bool {
    module.body.iter().any(|item| match item {
        swc_core::ecma::ast::ModuleItem::Stmt(stmt) => stmt_binds_infinity(stmt),
        swc_core::ecma::ast::ModuleItem::ModuleDecl(decl) => match decl {
            swc_core::ecma::ast::ModuleDecl::Import(import) => import
                .specifiers.iter().any(|spec| spec.local().sym == "Infinity"),
            _ => false,
        },
    })
}

fn body_binds_infinity(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_binds_infinity)
}

fn stmt_binds_infinity(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Decl(Decl::Var(var)) => var.decls.iter().any(|d| pat_binds_infinity(&d.name)),
        Stmt::Decl(Decl::Fn(FnDecl { ident, .. }))
        | Stmt::Decl(Decl::Class(ClassDecl { ident, .. })) => ident.sym == "Infinity",
        _ => false,
    }
}

fn pat_binds_infinity(pat: &swc_core::ecma::ast::Pat) -> bool {
    match pat {
        swc_core::ecma::ast::Pat::Ident(binding) => binding.id.sym == "Infinity",
        swc_core::ecma::ast::Pat::Rest(rest) => pat_binds_infinity(&rest.arg),
        swc_core::ecma::ast::Pat::Assign(assign) => pat_binds_infinity(&assign.left),
        swc_core::ecma::ast::Pat::Array(array) =>
            array.elems.iter().flatten().any(pat_binds_infinity),
        swc_core::ecma::ast::Pat::Object(object) => object.props.iter().any(|prop| match prop {
            swc_core::ecma::ast::ObjectPatProp::Assign(assign) => assign.key.sym == "Infinity",
            swc_core::ecma::ast::ObjectPatProp::KeyValue(prop) => pat_binds_infinity(&prop.value),
            swc_core::ecma::ast::ObjectPatProp::Rest(rest) => pat_binds_infinity(&rest.arg),
        }),
        swc_core::ecma::ast::Pat::Expr(_) | swc_core::ecma::ast::Pat::Invalid(_) => false,
    }
}

struct DynamicScopeFinder {
    unresolved_mark: Mark,
    found: bool,
}

impl Visit for DynamicScopeFinder {
    fn visit_with_stmt(&mut self, _: &WithStmt) {
        self.found = true;
    }

    fn visit_call_expr(&mut self, call: &CallExpr) {
        if let Callee::Expr(callee) = &call.callee {
            if let Expr::Ident(id) = callee.as_ref() {
                if is_unresolved_ident(id, "eval", self.unresolved_mark) {
                    self.found = true;
                }
            }
        }
        call.visit_children_with(self);
    }

    fn visit_function(&mut self, _: &Function) {}
    fn visit_arrow_expr(&mut self, _: &ArrowExpr) {}
    fn visit_fn_expr(&mut self, _: &FnExpr) {}
    fn visit_class_expr(&mut self, _: &ClassExpr) {}
}

fn has_dynamic_scope<T: VisitWith<DynamicScopeFinder>>(node: &T, unresolved_mark: Mark) -> bool {
    let mut finder = DynamicScopeFinder {
        unresolved_mark,
        found: false,
    };
    node.visit_with(&mut finder);
    finder.found
}
