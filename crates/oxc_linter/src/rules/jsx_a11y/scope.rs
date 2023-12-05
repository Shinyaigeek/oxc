use oxc_ast::{
    ast::{JSXAttributeItem, JSXElementName},
    AstKind,
};
use oxc_diagnostics::{
    miette::{self, Diagnostic},
    thiserror::Error,
};
use oxc_macros::declare_oxc_lint;
use oxc_span::Span;

use crate::{context::LintContext, rule::Rule, utils::has_jsx_prop_lowercase, AstNode};

#[derive(Debug, Default, Clone)]
pub struct Scope;

declare_oxc_lint!(
    /// ### What it does
    ///
    /// The scope prop should be used only on <th> elements.
    ///
    /// ### Why is this bad?
    /// The scope attribute makes table navigation much easier for screen reader users, provided that it is used correctly.
    /// Incorrectly used, scope can make table navigation much harder and less efficient.
    /// A screen reader operates under the assumption that a table has a header and that this header specifies a scope. Because of the way screen readers function, having an accurate header makes viewing a table far more accessible and more efficient for people who use the device.
    ///
    /// ### Example
    /// ```javascript
    /// // Bad
    /// <div scope />
    ///
    /// // Good
    /// <th scope="col" />
    /// <th scope={scope} />
    /// ```
    Scope,
    correctness
);

#[derive(Debug, Error, Diagnostic)]
#[error("eslint-plugin-jsx-a11y(scope): The scope prop can only be used on <th> elements")]
#[diagnostic(severity(warning), help("Must use scope prop only on <th> elements"))]
struct ScopeDiagnostic(#[label] pub Span);

impl Rule for Scope {
    fn run<'a>(&self, node: &AstNode<'a>, ctx: &LintContext<'a>) {
        let AstKind::JSXOpeningElement(jsx_el) = node.kind() else {
            return;
        };

        let scope_attribute = match has_jsx_prop_lowercase(jsx_el, "scope") {
            Some(v) => match v {
                JSXAttributeItem::Attribute(attr) => attr,
                JSXAttributeItem::SpreadAttribute(_) => {
                    return;
                }
            },
            None => {
                return;
            }
        };

        let JSXElementName::Identifier(identifier) = &jsx_el.name else {
            return;
        };

        let name = identifier.name.as_str();
        if name == "th" {
            return;
        }

        ctx.diagnostic(ScopeDiagnostic(scope_attribute.span));
    }
}

#[test]
fn test() {
    use crate::tester::Tester;

    let pass = vec![
        (r"<div />;", None),
        (r"<div foo />;", None),
        (r"<th scope />", None),
        (r"<th scope='row' />", None),
        (r"<th scope={foo} />", None),
        (r"<th scope={'col'} {...props} />", None),
        // TODO aria-query like parts is needed
        // (r"<Foo scope='bar' {...props} />", None),
        // TODO: When polymorphic components are supported
        // (r"<TableHeader scope="row" />", None)
    ];

    let fail = vec![
        (r"<div scope />", None),
        // TODO: When polymorphic components are supported
        // (r"<Foo scope='bar' />;", None),
    ];

    Tester::new(Scope::NAME, pass, fail).with_jsx_a11y_plugin(true).test_and_snapshot();
}
