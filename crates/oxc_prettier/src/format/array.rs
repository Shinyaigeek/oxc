use oxc_ast::ast::*;
use oxc_span::Span;
use oxc_syntax::operator::UnaryOperator;

use crate::{
    array,
    comments::DanglingCommentsPrintOptions,
    doc::{Doc, DocBuilder, Fill, Group},
    group, if_break, indent, line, softline, ss, Prettier,
};

use super::Format;

#[allow(clippy::enum_variant_names)]
pub enum Array<'a, 'b> {
    ArrayExpression(&'b ArrayExpression<'a>),
    TSTupleType(&'b TSTupleType<'a>),
    ArrayPattern(&'b ArrayPattern<'a>),
    ArrayAssignmentTarget(&'b ArrayAssignmentTarget<'a>),
}

impl<'a, 'b> Array<'a, 'b> {
    fn len(&self) -> usize {
        match self {
            Self::ArrayExpression(array) => array.elements.len(),
            Self::TSTupleType(tuple) => tuple.element_types.len(),
            Self::ArrayPattern(array) => array.elements.len(),
            Self::ArrayAssignmentTarget(array) => array.elements.len(),
        }
    }
    fn span(&self) -> Span {
        match self {
            Self::ArrayExpression(array) => array.span,
            Self::TSTupleType(tuple) => tuple.span,
            Self::ArrayPattern(array) => array.span,
            Self::ArrayAssignmentTarget(array) => array.span,
        }
    }
    fn is_concisely_printed(&self) -> bool {
        match self {
            Self::ArrayExpression(array) => {
                if array.elements.len() <= 1 {
                    return false;
                }

                return array.elements.iter().all(|element| {
                    let ArrayExpressionElement::Expression(expr) = element else {
                        return false;
                    };

                    match expr {
                        Expression::NumberLiteral(_) => true,
                        Expression::UnaryExpression(unary_expr) => {
                            matches!(
                                unary_expr.operator,
                                UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation
                            ) && matches!(unary_expr.argument, Expression::NumberLiteral(_))
                        }
                        _ => false,
                    }
                });
            }
            Self::ArrayPattern(_) | Self::ArrayAssignmentTarget(_) | Self::TSTupleType(_) => false,
        }
    }
}

pub(super) fn print_array<'a>(p: &mut Prettier<'a>, array: &Array<'a, '_>) -> Doc<'a> {
    if array.len() == 0 {
        return print_empty_array_elements(p, array);
    }

    let (needs_forced_trailing_comma, can_have_trailing_comma) =
        if let Array::ArrayExpression(array) = array {
            array.elements.last().map_or((false, false), |last| {
                (
                    matches!(last, ArrayExpressionElement::Elision(_)),
                    !matches!(last, ArrayExpressionElement::SpreadElement(_)),
                )
            })
        } else {
            (false, false)
        };

    let id = p.next_id();
    let should_use_concise_formatting = array.is_concisely_printed();

    let trailing_comma_fn = |p: &Prettier<'a>| {
        if !can_have_trailing_comma {
            ss!("")
        } else if needs_forced_trailing_comma {
            ss!(",")
        } else if should_use_concise_formatting {
            if_break!(p, ",", "", Some(id))
        } else {
            if_break!(p, ",", "", None)
        }
    };

    let mut parts = p.vec();
    let elements = if should_use_concise_formatting {
        print_array_elements_concisely(p, array, trailing_comma_fn)
    } else {
        let trailing_comma = trailing_comma_fn(p);
        array!(p, print_elements(p, array), trailing_comma)
    };
    let parts_inner = if let Some(dangling_comments) = p.print_dangling_comments(array.span(), None)
    {
        indent!(p, softline!(), elements, dangling_comments)
    } else {
        indent!(p, softline!(), elements)
    };
    parts.push(ss!("["));
    parts.push(parts_inner);
    parts.push(softline!());
    parts.push(ss!("]"));
    let should_break = should_break(array);
    Doc::Group(Group::new(parts, should_break).with_id(id))
}

fn print_empty_array_elements<'a>(p: &mut Prettier<'a>, array: &Array<'a, '_>) -> Doc<'a> {
    let dangling_options = DanglingCommentsPrintOptions::default().with_ident(true);
    p.print_dangling_comments(array.span(), Some(dangling_options)).map_or_else(
        || ss!("[]"),
        |dangling_comments| group![p, ss!("["), dangling_comments, softline!(), ss!("]")],
    )
}

fn print_elements<'a>(p: &mut Prettier<'a>, array: &Array<'a, '_>) -> Doc<'a> {
    let mut parts = p.vec();
    match array {
        Array::ArrayExpression(array) => {
            for (i, element) in array.elements.iter().enumerate() {
                if i > 0 && i < array.elements.len() {
                    parts.push(ss!(","));
                    parts.push(line!());
                }

                parts.push(element.format(p));
            }
        }
        Array::TSTupleType(tuple) => {
            for (i, element) in tuple.element_types.iter().enumerate() {
                if i > 0 && i < tuple.element_types.len() {
                    parts.push(ss!(","));
                    parts.push(line!());
                }

                parts.push(element.format(p));
            }
        }
        Array::ArrayPattern(array_pat) => {
            for (i, element) in array_pat.elements.iter().enumerate() {
                if i > 0 && i < array_pat.elements.len() {
                    parts.push(ss!(","));
                    parts.push(line!());
                }

                if let Some(binding_pat) = element {
                    parts.push(binding_pat.format(p));
                }
            }

            if let Some(rest) = &array_pat.rest {
                parts.push(ss!(","));
                parts.push(line!());
                parts.push(rest.format(p));
            }
        }
        Array::ArrayAssignmentTarget(array_pat) => {
            for (i, element) in array_pat.elements.iter().enumerate() {
                if i > 0 && i < array_pat.elements.len() {
                    parts.push(ss!(","));
                    parts.push(line!());
                }

                if let Some(binding_pat) = element {
                    parts.push(binding_pat.format(p));
                }
            }

            if let Some(rest) = &array_pat.rest {
                parts.push(ss!(","));
                parts.push(line!());
                parts.push(rest.format(p));
            }
        }
    }

    Doc::Array(parts)
}

fn print_array_elements_concisely<'a, F>(
    p: &mut Prettier<'a>,
    array: &Array<'a, '_>,
    trailing_comma_fn: F,
) -> Doc<'a>
where
    F: Fn(&Prettier<'a>) -> Doc<'a>,
{
    let mut parts = p.vec();
    match array {
        Array::ArrayExpression(array) => {
            for (i, element) in array.elements.iter().enumerate() {
                let is_last = i == array.elements.len() - 1;
                let part = if is_last {
                    array!(p, element.format(p), trailing_comma_fn(p))
                } else {
                    array!(p, element.format(p), ss!(","))
                };
                parts.push(part);

                if !is_last {
                    parts.push(line!());
                }
            }
        }
        _ => {
            // TODO: implement
            array!(p, print_elements(p, array), trailing_comma_fn(p));
        }
    }

    Doc::Fill(Fill::new(parts))
}

fn should_break(array: &Array) -> bool {
    if array.len() <= 1 {
        return false;
    }

    match array {
        Array::ArrayExpression(array) => {
            array.elements.iter().enumerate().all(|(index, element)| {
                let ArrayExpressionElement::Expression(element) = element else {
                    return false;
                };
                if let Some(ArrayExpressionElement::Expression(next_element)) =
                    array.elements.get(index + 1)
                {
                    let all_array_or_object = matches!(
                        (element, next_element),
                        (Expression::ArrayExpression(_), Expression::ArrayExpression(_))
                            | (Expression::ObjectExpression(_), Expression::ObjectExpression(_))
                    );
                    if !all_array_or_object {
                        return false;
                    }
                }

                match element {
                    Expression::ArrayExpression(array) => array.elements.len() > 1,
                    Expression::ObjectExpression(object) => object.properties.len() > 1,
                    _ => false,
                }
            })
        }
        Array::TSTupleType(tuple) => {
            tuple.element_types.iter().enumerate().all(|(index, element)| {
                let TSTupleElement::TSType(element) = element else { return false };

                if let Some(TSTupleElement::TSType(next_element)) =
                    tuple.element_types.get(index + 1)
                {
                    if !matches!(
                        (element, next_element),
                        (TSType::TSTupleType(_), TSType::TSTupleType(_))
                    ) {
                        return false;
                    }
                }

                let TSType::TSTupleType(array) = element else {
                    return false;
                };

                array.element_types.len() > 1
            })
        }
        Array::ArrayPattern(array) => false,
        Array::ArrayAssignmentTarget(array) => false,
    }
}
