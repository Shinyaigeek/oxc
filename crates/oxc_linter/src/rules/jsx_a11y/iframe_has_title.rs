use oxc_ast::{
    ast::{Expression, JSXAttributeValue, JSXElementName, JSXExpression, JSXExpressionContainer},
    AstKind,
};
use oxc_diagnostics::{
    miette::{self, Diagnostic},
    thiserror::Error,
};
use oxc_macros::declare_oxc_lint;
use oxc_span::Span;

use crate::{
    context::LintContext,
    rule::Rule,
    utils::{get_prop_value, has_jsx_prop_lowercase},
    AstNode,
};

#[derive(Debug, Error, Diagnostic)]
#[error("eslint(iframe-has-title): Missing `title` attribute for the `iframe` element.")]
#[diagnostic(severity(warning), help("Provide title property for iframe element."))]
struct IframeHasTitleDiagnostic(#[label] pub Span);

#[derive(Debug, Default, Clone)]
pub struct IframeHasTitle;

declare_oxc_lint!(
    /// ### What it does
    ///
    /// Enforce iframe elements have a title attribute.
    ///
    /// ### Why is this necessary?
    ///
    /// Screen reader users rely on a iframe title to describe the contents of the iframe.
    /// Navigating through iframe and iframe elements quickly becomes difficult and confusing for users of this technology if the markup does not contain a title attribute.
    ///
    /// ### What it checks
    ///
    /// This rule checks for title property on iframe element.
    ///
    /// ### Example
    /// ```javascript
    /// // Bad
    /// <iframe />
    /// <iframe {...props} />
    /// <iframe title="" />
    /// <iframe title={''} />
    /// <iframe title={``} />
    /// <iframe title={undefined} />
    /// <iframe title={false} />
    /// <iframe title={true} />
    /// <iframe title={42} />
    ///
    /// // Good
    /// <iframe title="This is a unique title" />
    /// <iframe title={uniqueTitle} />
    /// ```
    IframeHasTitle,
    correctness
);

impl Rule for IframeHasTitle {
    fn run<'a>(&self, node: &AstNode<'a>, ctx: &LintContext<'a>) {
        let AstKind::JSXOpeningElement(jsx_el) = node.kind() else {
            return;
        };

        let JSXElementName::Identifier(iden) = &jsx_el.name else {
            return;
        };

        let name = iden.name.as_str();

        if name != "iframe" {
            return;
        }

        let alt_prop = if let Some(prop) = has_jsx_prop_lowercase(jsx_el, "title") {
            prop
        } else {
            ctx.diagnostic(IframeHasTitleDiagnostic(jsx_el.span));
            return;
        };

        match get_prop_value(alt_prop) {
            Some(JSXAttributeValue::StringLiteral(str)) => {
                if !str.value.as_str().is_empty() {
                    return;
                }
            }
            Some(JSXAttributeValue::ExpressionContainer(JSXExpressionContainer {
                expression: JSXExpression::Expression(expr),
                ..
            })) => {
                if expr.is_string_literal() {
                    if let Expression::StringLiteral(str) = expr {
                        if !str.value.as_str().is_empty() {
                            return;
                        }
                    }
                    if let Expression::TemplateLiteral(tmpl) = expr {
                        if !tmpl.quasis.is_empty()
                            & !tmpl.expressions.is_empty()
                            & tmpl.quasis.iter().any(|q| !q.value.raw.as_str().is_empty())
                        {
                            return;
                        }
                    }
                }

                if expr.is_identifier_reference() & !expr.is_undefined() {
                    return;
                }
            }
            _ => {}
        }

        ctx.diagnostic(IframeHasTitleDiagnostic(jsx_el.span));
    }
}

#[test]
fn test() {
    use crate::tester::Tester;

    let pass = vec![
        // DEFAULT ELEMENT TESTS
        (r"<div />;", None),
        (r"<iframe title='Unique title' />", None),
        (r"<iframe title={foo} />", None),
        (r"<FooComponent />", None),
        // TODO: When polymorphic components are supported
        // CUSTOM ELEMENT TESTS FOR COMPONENTS SETTINGS
        // (r"<FooComponent title='Unique title' />", None),
    ];

    let fail = vec![
        // DEFAULT ELEMENT TESTS
        (r"<iframe />", None),
        (r"<iframe {...props} />", None),
        (r"<iframe title={undefined} />", None),
        (r"<iframe title='' />", None),
        (r"<iframe title={false} />", None),
        (r"<iframe title={true} />", None),
        (r"<iframe title={''} />", None),
        (r"<iframe title={``} />", None),
        (r"<iframe title={42} />", None),
        // TODO: When polymorphic components are supported
        // CUSTOM ELEMENT TESTS FOR COMPONENTS SETTINGS
        // (r"<FooComponent />", None),
    ];

    Tester::new(IframeHasTitle::NAME, pass, fail).with_jsx_a11y_plugin(true).test_and_snapshot();
}
