//! Direct port of needs-parens for adding or removing parentheses.
//!
//! See <https://github.com/prettier/prettier/blob/main/src/language-js/needs-parens.js>

#![allow(
    clippy::unused_self,
    clippy::match_same_arms,
    clippy::match_like_matches_macro,
    clippy::single_match
)]
use oxc_ast::{
    ast::{
        AssignmentTarget, AssignmentTargetPattern, ChainElement, ExportDefaultDeclarationKind,
        Expression, ModuleDeclaration, ObjectExpression, SimpleAssignmentTarget,
    },
    AstKind,
};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{BinaryOperator, UnaryOperator, UpdateOperator};

use crate::{array, doc::Doc, ss, Prettier};

impl<'a> Prettier<'a> {
    pub(crate) fn wrap_parens(&mut self, doc: Doc<'a>, kind: AstKind<'a>) -> Doc<'a> {
        if self.need_parens(kind) {
            array![self, ss!("("), doc, ss!(")")]
        } else {
            doc
        }
    }

    fn need_parens(&mut self, kind: AstKind<'a>) -> bool {
        if matches!(kind, AstKind::Program(_)) {
            return false;
        }

        if kind.is_statement() || kind.is_declaration() {
            return false;
        }

        let parent_kind = self.parent_kind();

        if let AstKind::ObjectExpression(e) = kind {
            if self.check_object_expression(e) {
                return true;
            }
        }

        if self.check_parent_kind(kind, parent_kind) {
            return true;
        }

        if self.check_kind(kind, parent_kind) {
            return true;
        }

        false
    }

    fn check_kind(&self, kind: AstKind<'a>, parent_kind: AstKind<'a>) -> bool {
        match kind {
            AstKind::NumberLiteral(literal) => {
                matches!(parent_kind, AstKind::MemberExpression(e) if e.object().span() == literal.span)
            }
            AstKind::SequenceExpression(_) => !matches!(parent_kind, AstKind::Program(_)),
            AstKind::ObjectExpression(e) => self.check_object_function_class(e.span),
            AstKind::Function(f) if f.is_expression() => {
                if self.check_object_function_class(f.span) {
                    return true;
                }
                match parent_kind {
                    AstKind::CallExpression(call_expr) => call_expr.callee.span() == f.span,
                    AstKind::NewExpression(new_expr) => new_expr.callee.span() == f.span,
                    AstKind::TaggedTemplateExpression(_) => true,
                    _ => false,
                }
            }
            AstKind::Class(c) if c.is_expression() => self.check_object_function_class(c.span),
            AstKind::AssignmentExpression(assign_expr) => match parent_kind {
                AstKind::ArrowExpression(arrow_expr)
                    if arrow_expr
                        .get_expression()
                        .is_some_and(|e| e.span() == assign_expr.span) =>
                {
                    true
                }
                AstKind::AssignmentExpression(_) => false,
                AstKind::ForStatement(stmt)
                    if stmt.init.as_ref().is_some_and(|e| e.span() == assign_expr.span)
                        || stmt.update.as_ref().is_some_and(|e| e.span() == assign_expr.span) =>
                {
                    false
                }
                AstKind::ExpressionStatement(_) => matches!(
                    assign_expr.left,
                    AssignmentTarget::AssignmentTargetPattern(
                        AssignmentTargetPattern::ObjectAssignmentTarget(_)
                    )
                ),
                _ => true,
            },
            AstKind::UpdateExpression(update_expr) => match parent_kind {
                AstKind::UnaryExpression(unary_expr) => {
                    update_expr.prefix
                        && ((update_expr.operator == UpdateOperator::Increment
                            && unary_expr.operator == UnaryOperator::UnaryPlus)
                            || (update_expr.operator == UpdateOperator::Decrement
                                && unary_expr.operator == UnaryOperator::UnaryNegation))
                }
                _ => self.check_update_unary(update_expr.span),
            },
            AstKind::UnaryExpression(unary_expr) => match parent_kind {
                AstKind::UnaryExpression(parent_expr) => {
                    let u_op = unary_expr.operator;
                    u_op == parent_expr.operator
                        && (matches!(u_op, UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation))
                }
                _ => self.check_update_unary(unary_expr.span),
            },
            AstKind::YieldExpression(e) => match parent_kind {
                AstKind::AwaitExpression(_) => true,
                _ => self.check_yield_await(e.span),
            },
            AstKind::AwaitExpression(e) => self.check_yield_await(e.span),
            AstKind::TSTypeAssertion(e) => self.check_binarish(e.span),
            AstKind::TSAsExpression(e) => self.check_binarish(e.span),
            AstKind::TSSatisfiesExpression(e) => self.check_binarish(e.span),
            AstKind::LogicalExpression(e) => self.check_binarish(e.span),
            AstKind::BinaryExpression(e) => match parent_kind {
                AstKind::UpdateExpression(_) => true,
                _ if e.operator == BinaryOperator::In
                    && self.is_path_in_for_statement_initializer(e.span) =>
                {
                    true
                }
                _ => self.check_binarish(e.span),
            },
            AstKind::MemberExpression(e) => self.check_member_call(e.span()),
            AstKind::CallExpression(e) => self.check_member_call(e.span),
            AstKind::TaggedTemplateExpression(e) => {
                self.check_member_call_tagged_template_ts_non_null(e.span)
            }
            AstKind::TSNonNullExpression(e) => {
                self.check_member_call_tagged_template_ts_non_null(e.span)
            }
            AstKind::ConditionalExpression(e) => match parent_kind {
                AstKind::TaggedTemplateExpression(_)
                | AstKind::UnaryExpression(_)
                | AstKind::SpreadElement(_)
                | AstKind::BinaryExpression(_)
                | AstKind::LogicalExpression(_)
                | AstKind::ModuleDeclaration(ModuleDeclaration::ExportDefaultDeclaration(_))
                | AstKind::AwaitExpression(_)
                | AstKind::JSXSpreadAttribute(_)
                | AstKind::TSAsExpression(_)
                | AstKind::TSSatisfiesExpression(_)
                | AstKind::TSNonNullExpression(_) => true,
                AstKind::CallExpression(call_expr) => call_expr.callee.span() == e.span,
                AstKind::NewExpression(new_expr) => new_expr.callee.span() == e.span,
                AstKind::ConditionalExpression(cond_expr) => cond_expr.test.span() == e.span,
                AstKind::MemberExpression(member_expr) => member_expr.object().span() == e.span,
                _ => false,
            },
            AstKind::Function(e) if e.is_expression() => match parent_kind {
                AstKind::CallExpression(call_expr) => call_expr.callee.span() == e.span,
                AstKind::NewExpression(new_expr) => new_expr.callee.span() == e.span,
                AstKind::TaggedTemplateExpression(_) => true,
                _ => false,
            },
            AstKind::ArrowExpression(e) => match parent_kind {
                AstKind::CallExpression(call_expr) => call_expr.callee.span() == e.span,
                AstKind::NewExpression(new_expr) => new_expr.callee.span() == e.span,
                AstKind::MemberExpression(member_expr) => member_expr.object().span() == e.span,
                AstKind::TSAsExpression(_)
                | AstKind::TSSatisfiesExpression(_)
                | AstKind::TSNonNullExpression(_)
                | AstKind::TaggedTemplateExpression(_)
                | AstKind::UnaryExpression(_)
                | AstKind::LogicalExpression(_)
                | AstKind::AwaitExpression(_)
                | AstKind::TSTypeAssertion(_) => true,
                AstKind::ConditionalExpression(cond_expr) => cond_expr.test.span() == e.span,
                _ => false,
            },
            AstKind::Class(class) if class.is_expression() => match parent_kind {
                AstKind::NewExpression(new_expr) => new_expr.callee.span() == class.span,
                _ => false,
            },
            _ => false,
        }
    }

    fn check_parent_kind(&mut self, kind: AstKind<'a>, parent_kind: AstKind<'a>) -> bool {
        match parent_kind {
            AstKind::Class(class) => {
                if let Some(h) = &class.super_class {
                    match kind {
                        AstKind::ArrowExpression(e) if e.span == h.span() => return true,
                        AstKind::AssignmentExpression(e) if e.span == h.span() => return true,
                        AstKind::AwaitExpression(e) if e.span == h.span() => return true,
                        AstKind::BinaryExpression(e) if e.span == h.span() => return true,
                        AstKind::ConditionalExpression(e) if e.span == h.span() => return true,
                        AstKind::LogicalExpression(e) if e.span == h.span() => return true,
                        AstKind::NewExpression(e) if e.span == h.span() => return true,
                        AstKind::ObjectExpression(e) if e.span == h.span() => return true,
                        AstKind::SequenceExpression(e) if e.span == h.span() => return true,
                        AstKind::TaggedTemplateExpression(e) if e.span == h.span() => return true,
                        AstKind::UnaryExpression(e) if e.span == h.span() => return true,
                        AstKind::UpdateExpression(e) if e.span == h.span() => return true,
                        AstKind::YieldExpression(e) if e.span == h.span() => return true,
                        AstKind::TSNonNullExpression(e) if e.span == h.span() => return true,
                        AstKind::Class(e)
                            if e.is_expression()
                                && !e.decorators.is_empty()
                                && e.span == h.span() =>
                        {
                            return true
                        }
                        _ => {}
                    }
                }
            }
            AstKind::ModuleDeclaration(ModuleDeclaration::ExportDefaultDeclaration(decl)) => {
                if let ExportDefaultDeclarationKind::Expression(e) = &decl.declaration {
                    return matches!(e, Expression::SequenceExpression(_))
                        || self.should_wrap_function_for_export_default();
                }
            }
            _ => {}
        }
        false
    }

    fn check_object_expression(&self, obj_expr: &ObjectExpression<'a>) -> bool {
        let mut arrow_expr = None;
        for kind in self.nodes.iter().rev() {
            if let AstKind::ArrowExpression(e) = kind {
                e.get_expression();
                arrow_expr = Some(e);
                break;
            }
        }
        if let Some(arrow_expr) = arrow_expr {
            if let Some(e) = arrow_expr.get_expression() {
                if !matches!(
                    e,
                    Expression::SequenceExpression(_) | Expression::AssignmentExpression(_)
                ) && Self::starts_with_no_lookahead_token(e, obj_expr.span)
                {
                    return true;
                }
            }
        }
        false
    }

    fn check_object_function_class(&self, span: Span) -> bool {
        for ast_kind in self.nodes.iter().rev() {
            if let AstKind::ExpressionStatement(e) = ast_kind {
                if Self::starts_with_no_lookahead_token(&e.expression, span) {
                    return true;
                }
            }
        }
        false
    }

    fn check_update_unary(&self, span: Span) -> bool {
        match self.parent_kind() {
            AstKind::MemberExpression(member_expr) => member_expr.object().span() == span,
            AstKind::TaggedTemplateExpression(_) => true,
            AstKind::CallExpression(call_expr) => call_expr.callee.span() == span,
            AstKind::NewExpression(new_expr) => new_expr.callee.span() == span,
            AstKind::BinaryExpression(bin_expr) => {
                bin_expr.left.span() == span && bin_expr.operator == BinaryOperator::Exponential
            }
            AstKind::TSNonNullExpression(_) => true,
            _ => false,
        }
    }

    fn check_yield_await(&self, span: Span) -> bool {
        match self.parent_kind() {
            AstKind::TaggedTemplateExpression(_)
            | AstKind::UnaryExpression(_)
            | AstKind::LogicalExpression(_)
            | AstKind::SpreadElement(_)
            | AstKind::TSAsExpression(_)
            | AstKind::TSSatisfiesExpression(_)
            | AstKind::TSNonNullExpression(_)
            | AstKind::BinaryExpression(_) => true,
            AstKind::MemberExpression(member_expr) => member_expr.object().span() == span,
            AstKind::NewExpression(new_expr) => new_expr.callee.span() == span,
            AstKind::CallExpression(new_expr) => new_expr.callee.span() == span,
            AstKind::ConditionalExpression(con_expr) => con_expr.test.span() == span,
            _ => false,
        }
    }

    fn check_binarish(&self, span: Span) -> bool {
        match self.parent_kind() {
            AstKind::TSAsExpression(_) => !self.is_binary_cast_expression(span),
            AstKind::TSSatisfiesExpression(_) => !self.is_binary_cast_expression(span),
            AstKind::ConditionalExpression(_) => self.is_binary_cast_expression(span),
            AstKind::NewExpression(new_expr) => new_expr.callee.span() == span,
            AstKind::CallExpression(new_expr) => new_expr.callee.span() == span,
            AstKind::Class(class) => class.super_class.as_ref().is_some_and(|e| e.span() == span),
            AstKind::TSTypeAssertion(_)
            | AstKind::TaggedTemplateExpression(_)
            | AstKind::UnaryExpression(_)
            | AstKind::JSXSpreadAttribute(_)
            | AstKind::SpreadElement(_)
            | AstKind::AwaitExpression(_)
            | AstKind::TSNonNullExpression(_)
            | AstKind::UpdateExpression(_) => true,
            AstKind::MemberExpression(member_expr) => member_expr.object().span() == span,
            AstKind::AssignmentExpression(assign_expr) => {
                assign_expr.left.span() == span && self.is_binary_cast_expression(span)
            }
            AstKind::AssignmentPattern(assign_pat) => {
                assign_pat.left.span() == span && self.is_binary_cast_expression(span)
            }
            _ => false,
        }
    }

    fn check_member_call(&self, span: Span) -> bool {
        // if (shouldAddParenthesesToChainElement(path)) {
        // return true;
        // }
        self.check_member_call_tagged_template_ts_non_null(span)
    }

    fn check_member_call_tagged_template_ts_non_null(&self, span: Span) -> bool {
        match self.parent_kind() {
            AstKind::NewExpression(new_expr) if new_expr.callee.span() == span => {
                let mut object = &new_expr.callee;
                loop {
                    match object {
                        Expression::CallExpression(_) => return true,
                        Expression::MemberExpression(e) => {
                            object = e.object();
                        }
                        Expression::TaggedTemplateExpression(e) => {
                            object = &e.tag;
                        }
                        Expression::TSNonNullExpression(e) => {
                            object = &e.expression;
                        }
                        _ => return false,
                    }
                }
            }
            _ => false,
        }
    }

    fn should_wrap_function_for_export_default(&mut self) -> bool {
        let kind = self.current_kind();
        let b = matches!(
            self.parent_kind(),
            AstKind::ModuleDeclaration(ModuleDeclaration::ExportDefaultDeclaration(_))
        );
        if matches!(kind, AstKind::Function(f) if f.is_expression())
            || matches!(kind, AstKind::Class(c) if c.is_expression())
        {
            return b || !self.need_parens(self.current_kind());
        }

        if !Self::has_naked_left_side(kind) || (!b && self.need_parens(self.current_kind())) {
            return false;
        }

        let lhs = Self::get_left_side_path_name(kind);
        self.nodes.push(lhs);
        let result = self.should_wrap_function_for_export_default();
        self.nodes.pop();
        result
    }

    fn has_naked_left_side(kind: AstKind<'a>) -> bool {
        matches!(
            kind,
            AstKind::AssignmentExpression(_)
                | AstKind::BinaryExpression(_)
                | AstKind::LogicalExpression(_)
                | AstKind::ConditionalExpression(_)
                | AstKind::CallExpression(_)
                | AstKind::MemberExpression(_)
                | AstKind::SequenceExpression(_)
                | AstKind::TaggedTemplateExpression(_)
                | AstKind::TSNonNullExpression(_)
                | AstKind::ChainExpression(_)
        ) || matches!(kind, AstKind::UpdateExpression(e) if !e.prefix)
    }

    fn get_left_side_path_name(kind: AstKind<'a>) -> AstKind<'a> {
        match kind {
            AstKind::CallExpression(e) => AstKind::from_expression(&e.callee),
            AstKind::ConditionalExpression(e) => AstKind::from_expression(&e.test),
            AstKind::TaggedTemplateExpression(e) => AstKind::from_expression(&e.tag),
            AstKind::AssignmentExpression(e) => AstKind::AssignmentTarget(&e.left),
            AstKind::MemberExpression(e) => AstKind::from_expression(e.object()),
            _ => panic!("need to handle {}", kind.debug_name()),
        }
    }

    fn is_binary_cast_expression(&self, _span: Span) -> bool {
        false
    }

    fn is_path_in_for_statement_initializer(&self, span: Span) -> bool {
        let mut node = Some(span);
        let mut parents = self.nodes.iter().rev();
        while let Some(n) = node {
            let parent = parents.next();
            if let Some(AstKind::ForStatement(stmt)) = parent {
                if stmt.init.as_ref().is_some_and(|init| init.span() == n) {
                    return true;
                }
            }
            node = parent.map(GetSpan::span);
        }
        false
    }

    fn starts_with_no_lookahead_token(e: &Expression<'a>, span: Span) -> bool {
        match e {
            Expression::BinaryExpression(e) => Self::starts_with_no_lookahead_token(&e.left, span),
            Expression::LogicalExpression(e) => Self::starts_with_no_lookahead_token(&e.left, span),
            Expression::AssignmentExpression(e) => match &e.left {
                AssignmentTarget::SimpleAssignmentTarget(t) => match t {
                    SimpleAssignmentTarget::AssignmentTargetIdentifier(_) => false,
                    SimpleAssignmentTarget::MemberAssignmentTarget(e) => {
                        Self::starts_with_no_lookahead_token(e.object(), span)
                    }
                    SimpleAssignmentTarget::TSAsExpression(e) => {
                        Self::starts_with_no_lookahead_token(&e.expression, span)
                    }
                    SimpleAssignmentTarget::TSSatisfiesExpression(e) => {
                        Self::starts_with_no_lookahead_token(&e.expression, span)
                    }
                    SimpleAssignmentTarget::TSNonNullExpression(e) => {
                        Self::starts_with_no_lookahead_token(&e.expression, span)
                    }
                    SimpleAssignmentTarget::TSTypeAssertion(e) => {
                        Self::starts_with_no_lookahead_token(&e.expression, span)
                    }
                },
                AssignmentTarget::AssignmentTargetPattern(_) => false,
            },
            Expression::MemberExpression(e) => {
                Self::starts_with_no_lookahead_token(e.object(), span)
            }
            Expression::TaggedTemplateExpression(e) => {
                if matches!(e.tag, Expression::FunctionExpression(_)) {
                    return false;
                }
                Self::starts_with_no_lookahead_token(&e.tag, span)
            }
            Expression::CallExpression(e) => {
                if matches!(e.callee, Expression::FunctionExpression(_)) {
                    return false;
                }
                Self::starts_with_no_lookahead_token(&e.callee, span)
            }
            Expression::ConditionalExpression(e) => {
                Self::starts_with_no_lookahead_token(&e.test, span)
            }
            Expression::UpdateExpression(e) => {
                !e.prefix
                    && match &e.argument {
                        SimpleAssignmentTarget::AssignmentTargetIdentifier(_) => false,
                        SimpleAssignmentTarget::MemberAssignmentTarget(e) => {
                            Self::starts_with_no_lookahead_token(e.object(), span)
                        }
                        SimpleAssignmentTarget::TSAsExpression(e) => {
                            Self::starts_with_no_lookahead_token(&e.expression, span)
                        }
                        SimpleAssignmentTarget::TSSatisfiesExpression(e) => {
                            Self::starts_with_no_lookahead_token(&e.expression, span)
                        }
                        SimpleAssignmentTarget::TSNonNullExpression(e) => {
                            Self::starts_with_no_lookahead_token(&e.expression, span)
                        }
                        SimpleAssignmentTarget::TSTypeAssertion(e) => {
                            Self::starts_with_no_lookahead_token(&e.expression, span)
                        }
                    }
            }
            Expression::SequenceExpression(e) => e
                .expressions
                .get(0)
                .map_or(false, |e| Self::starts_with_no_lookahead_token(e, span)),
            Expression::ChainExpression(e) => match &e.expression {
                ChainElement::CallExpression(e) => {
                    Self::starts_with_no_lookahead_token(&e.callee, span)
                }
                ChainElement::MemberExpression(e) => {
                    Self::starts_with_no_lookahead_token(e.object(), span)
                }
            },
            Expression::TSSatisfiesExpression(e) => {
                Self::starts_with_no_lookahead_token(&e.expression, span)
            }
            Expression::TSAsExpression(e) => {
                Self::starts_with_no_lookahead_token(&e.expression, span)
            }
            Expression::TSNonNullExpression(e) => {
                Self::starts_with_no_lookahead_token(&e.expression, span)
            }
            _ => e.span() == span,
        }
    }
}
