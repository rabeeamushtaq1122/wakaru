use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{
    AssignExpr, CallExpr, Callee, Expr, Ident, IdentName, MemberExpr, MemberProp, Module,
    UnaryExpr, UnaryOp, UpdateExpr,
};
use swc_core::ecma::utils::ExprFactory;
use swc_core::ecma::visit::{Visit, VisitMut, VisitMutWith, VisitWith};

use super::RewriteLevel;
use crate::utils::paren::strip_parens;

pub struct UnHasOwnCall {
    level: RewriteLevel,
    unresolved_mark: Mark,
    blocked: bool,
}

impl UnHasOwnCall {
    pub fn new(level: RewriteLevel, unresolved_mark: Mark) -> Self {
        Self {
            level,
            unresolved_mark,
            blocked: false,
        }
    }
}

impl VisitMut for UnHasOwnCall {
    fn visit_mut_module(&mut self, module: &mut Module) {
        self.blocked = HasOwnMutationCollector::has_unsafe_write(module, self.unresolved_mark);
        module.visit_mut_children_with(self);
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        expr.visit_mut_children_with(self);
        if self.level != RewriteLevel::Aggressive || self.blocked {
            return;
        }
        let Expr::Call(call) = expr else { return };
        let Some(object) = match_has_own_call(call, self.unresolved_mark) else {
            return;
        };
        let callee = Expr::Member(MemberExpr {
            span: DUMMY_SP,
            obj: Box::new(Expr::Ident(object)),
            prop: MemberProp::Ident(IdentName::new("hasOwn".into(), DUMMY_SP)),
        });
        call.callee = callee.as_callee();
    }
}

fn match_has_own_call(call: &CallExpr, mark: Mark) -> Option<Ident> {
    if call.args.len() != 2 || call.args.iter().any(|arg| arg.spread.is_some()) {
        return None;
    }
    let Callee::Expr(callee) = &call.callee else {
        return None;
    };
    let Expr::Member(call_member) = strip_parens(callee) else {
        return None;
    };
    if !matches!(&call_member.prop, MemberProp::Ident(prop) if prop.sym.as_ref() == "call") {
        return None;
    }
    let Expr::Member(method_member) = strip_parens(call_member.obj.as_ref()) else {
        return None;
    };
    if !matches!(&method_member.prop, MemberProp::Ident(prop) if prop.sym.as_ref() == "hasOwnProperty")
    {
        return None;
    }
    let Expr::Member(proto_member) = strip_parens(method_member.obj.as_ref()) else {
        return None;
    };
    if !matches!(&proto_member.prop, MemberProp::Ident(prop) if prop.sym.as_ref() == "prototype") {
        return None;
    }
    let Expr::Ident(object) = strip_parens(proto_member.obj.as_ref()) else {
        return None;
    };
    if object.sym.as_ref() != "Object" || object.ctxt.outer() != mark {
        return None;
    }
    Some(object.clone())
}

struct HasOwnMutationCollector {
    mark: Mark,
    write_depth: usize,
    blocked: bool,
}

impl HasOwnMutationCollector {
    fn has_unsafe_write(module: &Module, mark: Mark) -> bool {
        let mut collector = Self {
            mark,
            write_depth: 0,
            blocked: false,
        };
        module.visit_with(&mut collector);
        collector.blocked
    }

    fn is_object(&self, expr: &Expr) -> bool {
        matches!(expr, Expr::Ident(id) if id.sym.as_ref() == "Object" && id.ctxt.outer() == self.mark)
    }

    fn is_target(&self, member: &MemberExpr) -> bool {
        let MemberProp::Ident(prop) = &member.prop else {
            return false;
        };
        if self.is_object(member.obj.as_ref()) {
            return prop.sym.as_ref() == "prototype";
        }
        let Expr::Member(parent) = member.obj.as_ref() else {
            return false;
        };
        let MemberProp::Ident(parent_prop) = &parent.prop else {
            return false;
        };
        self.is_object(parent.obj.as_ref())
            && ((parent_prop.sym.as_ref() == "prototype" && prop.sym.as_ref() == "hasOwnProperty")
                || (parent_prop.sym.as_ref() == "prototype" && prop.sym.as_ref() == "hasOwn"))
    }
}

impl Visit for HasOwnMutationCollector {
    fn visit_ident(&mut self, ident: &Ident) {
        if self.write_depth > 0 && ident.sym.as_ref() == "Object" && ident.ctxt.outer() == self.mark
        {
            self.blocked = true;
        }
    }

    fn visit_member_expr(&mut self, member: &MemberExpr) {
        if self.write_depth > 0 && self.is_target(member) {
            self.blocked = true;
        }
        member.visit_children_with(self);
    }

    fn visit_assign_expr(&mut self, expr: &AssignExpr) {
        self.write_depth += 1;
        expr.left.visit_with(self);
        self.write_depth -= 1;
        expr.right.visit_with(self);
    }

    fn visit_update_expr(&mut self, expr: &UpdateExpr) {
        self.write_depth += 1;
        expr.arg.visit_with(self);
        self.write_depth -= 1;
    }

    fn visit_unary_expr(&mut self, expr: &UnaryExpr) {
        if expr.op == UnaryOp::Delete {
            self.write_depth += 1;
        }
        expr.arg.visit_with(self);
        if expr.op == UnaryOp::Delete {
            self.write_depth -= 1;
        }
    }
}
