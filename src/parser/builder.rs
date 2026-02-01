/// AST builder - converts pest parse tree to AST
///
/// This module contains functions to convert pest's Pairs into our AST nodes.

use super::ast::*;
use super::{ParseError, ParseResult, Rule};
use pest::iterators::{Pair, Pairs};
use std::collections::HashMap;

/// Build AST from pest Pairs
pub fn build_ast(pairs: Pairs<Rule>) -> ParseResult<CypherQuery> {
    for pair in pairs {
        match pair.as_rule() {
            Rule::cypher_query => {
                return build_cypher_query(pair);
            }
            _ => {}
        }
    }
    Err(ParseError::InvalidSyntax("Empty query".into()))
}

fn build_cypher_query(pair: Pair<Rule>) -> ParseResult<CypherQuery> {
    let mut inner = pair.into_inner();
    let query_pair = inner
        .next()
        .ok_or_else(|| ParseError::InvalidSyntax("Missing query".into()))?;

    build_query(query_pair)
}

fn build_query(pair: Pair<Rule>) -> ParseResult<CypherQuery> {
    let mut inner = pair.into_inner();
    let first = inner
        .next()
        .ok_or_else(|| ParseError::InvalidSyntax("Empty query body".into()))?;

    match first.as_rule() {
        Rule::read_query => build_read_query(first),
        Rule::write_query => build_write_query(first),
        Rule::mixed_query => build_mixed_query(first),
        _ => Err(ParseError::InvalidSyntax(format!(
            "Unknown query type: {:?}",
            first.as_rule()
        ))),
    }
}

fn build_read_query(pair: Pair<Rule>) -> ParseResult<CypherQuery> {
    let mut match_clause = None;
    let mut where_clause = None;
    let mut return_clause = None;

    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::match_clause => {
                match_clause = Some(build_match_clause(inner_pair)?);
            }
            Rule::where_clause => {
                where_clause = Some(build_where_clause(inner_pair)?);
            }
            Rule::return_clause => {
                return_clause = Some(build_return_clause(inner_pair)?);
            }
            _ => {}
        }
    }

    Ok(CypherQuery::Read {
        match_clause: match_clause.ok_or_else(|| {
            ParseError::InvalidSyntax("MATCH clause required in read query".into())
        })?,
        where_clause,
        return_clause: return_clause.ok_or_else(|| {
            ParseError::InvalidSyntax("RETURN clause required in read query".into())
        })?,
    })
}

fn build_write_query(pair: Pair<Rule>) -> ParseResult<CypherQuery> {
    let inner_pair = pair
        .into_inner()
        .next()
        .ok_or_else(|| ParseError::InvalidSyntax("Empty write query".into()))?;

    let write_clause = match inner_pair.as_rule() {
        Rule::create_clause => build_create_clause(inner_pair)?,
        Rule::delete_clause => build_delete_clause(inner_pair)?,
        Rule::set_clause => build_set_clause(inner_pair)?,
        _ => {
            return Err(ParseError::InvalidSyntax(format!(
                "Unknown write clause: {:?}",
                inner_pair.as_rule()
            )))
        }
    };

    Ok(CypherQuery::Write(write_clause))
}

fn build_mixed_query(pair: Pair<Rule>) -> ParseResult<CypherQuery> {
    let mut match_clause = None;
    let mut where_clause = None;
    let mut write_clause = None;
    let mut return_clause = None;

    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::match_clause => {
                match_clause = Some(build_match_clause(inner_pair)?);
            }
            Rule::where_clause => {
                where_clause = Some(build_where_clause(inner_pair)?);
            }
            Rule::create_clause => {
                write_clause = Some(build_create_clause(inner_pair)?);
            }
            Rule::delete_clause => {
                write_clause = Some(build_delete_clause(inner_pair)?);
            }
            Rule::set_clause => {
                write_clause = Some(build_set_clause(inner_pair)?);
            }
            Rule::return_clause => {
                return_clause = Some(build_return_clause(inner_pair)?);
            }
            _ => {}
        }
    }

    Ok(CypherQuery::Mixed {
        match_clause: match_clause.ok_or_else(|| {
            ParseError::InvalidSyntax("MATCH clause required in mixed query".into())
        })?,
        where_clause,
        write_clause: write_clause.ok_or_else(|| {
            ParseError::InvalidSyntax("Write clause required in mixed query".into())
        })?,
        return_clause,
    })
}

fn build_match_clause(pair: Pair<Rule>) -> ParseResult<MatchClause> {
    let mut patterns = Vec::new();

    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::pattern {
            patterns.push(build_pattern(inner_pair)?);
        }
    }

    Ok(MatchClause { patterns })
}

fn build_create_clause(pair: Pair<Rule>) -> ParseResult<WriteClause> {
    let mut patterns = Vec::new();

    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::pattern {
            patterns.push(build_pattern(inner_pair)?);
        }
    }

    Ok(WriteClause::Create { patterns })
}

fn build_delete_clause(pair: Pair<Rule>) -> ParseResult<WriteClause> {
    let text = pair.as_str();
    let detach = text.to_uppercase().starts_with("DETACH");

    let mut expressions = Vec::new();

    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::expression {
            expressions.push(build_expression(inner_pair)?);
        }
    }

    Ok(WriteClause::Delete {
        expressions,
        detach,
    })
}

fn build_set_clause(pair: Pair<Rule>) -> ParseResult<WriteClause> {
    let mut items = Vec::new();

    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::set_item {
            items.push(build_set_item(inner_pair)?);
        }
    }

    Ok(WriteClause::Set { items })
}

fn build_set_item(pair: Pair<Rule>) -> ParseResult<SetItem> {
    let mut inner = pair.into_inner();

    let property = build_property_expression(
        inner
            .next()
            .ok_or_else(|| ParseError::InvalidSyntax("Missing property in SET item".into()))?,
    )?;

    let value = build_expression(
        inner
            .next()
            .ok_or_else(|| ParseError::InvalidSyntax("Missing value in SET item".into()))?,
    )?;

    Ok(SetItem { property, value })
}

fn build_where_clause(pair: Pair<Rule>) -> ParseResult<WhereClause> {
    let mut inner = pair.into_inner();
    let condition = build_expression(
        inner
            .next()
            .ok_or_else(|| ParseError::InvalidSyntax("Missing condition in WHERE".into()))?,
    )?;

    Ok(WhereClause { condition })
}

fn build_return_clause(pair: Pair<Rule>) -> ParseResult<ReturnClause> {
    let mut items = Vec::new();
    let mut order_by = None;
    let mut limit = None;

    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::return_item => {
                items.push(build_return_item(inner_pair)?);
            }
            Rule::order_by => {
                order_by = Some(build_order_by(inner_pair)?);
            }
            Rule::limit => {
                limit = Some(build_limit(inner_pair)?);
            }
            _ => {}
        }
    }

    Ok(ReturnClause {
        items,
        order_by,
        limit,
    })
}

fn build_return_item(pair: Pair<Rule>) -> ParseResult<ReturnItem> {
    let mut inner = pair.into_inner();

    let expression = build_expression(
        inner
            .next()
            .ok_or_else(|| ParseError::InvalidSyntax("Missing expression in RETURN item".into()))?,
    )?;

    let alias = inner.next().map(|p| p.as_str().to_string());

    Ok(ReturnItem { expression, alias })
}

fn build_order_by(pair: Pair<Rule>) -> ParseResult<Vec<SortItem>> {
    let mut items = Vec::new();

    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::sort_item {
            items.push(build_sort_item(inner_pair)?);
        }
    }

    Ok(items)
}

fn build_sort_item(pair: Pair<Rule>) -> ParseResult<SortItem> {
    let mut inner = pair.into_inner();

    let expression = build_expression(
        inner
            .next()
            .ok_or_else(|| ParseError::InvalidSyntax("Missing expression in sort item".into()))?,
    )?;

    let ascending = inner
        .next()
        .map(|p| p.as_str().to_uppercase() != "DESC")
        .unwrap_or(true);

    Ok(SortItem {
        expression,
        ascending,
    })
}

fn build_limit(pair: Pair<Rule>) -> ParseResult<i64> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ParseError::InvalidSyntax("Missing value in LIMIT".into()))?;

    inner
        .as_str()
        .parse::<i64>()
        .map_err(|_| ParseError::InvalidSyntax("Invalid LIMIT value".into()))
}

fn build_pattern(pair: Pair<Rule>) -> ParseResult<Pattern> {
    let mut elements = Vec::new();

    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::node_pattern => {
                elements.push(PatternElement::Node(build_node_pattern(inner_pair)?));
            }
            Rule::edge_pattern => {
                elements.push(PatternElement::Edge(build_edge_pattern(inner_pair)?));
            }
            _ => {}
        }
    }

    Ok(Pattern { elements })
}

fn build_node_pattern(pair: Pair<Rule>) -> ParseResult<NodePattern> {
    let mut variable = None;
    let mut label = None;
    let mut properties = None;

    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::identifier => {
                variable = Some(inner_pair.as_str().to_string());
            }
            Rule::label => {
                label = Some(inner_pair.as_str().to_string());
            }
            Rule::properties => {
                properties = Some(build_properties(inner_pair)?);
            }
            _ => {}
        }
    }

    Ok(NodePattern {
        variable,
        label,
        properties,
    })
}

fn build_edge_pattern(pair: Pair<Rule>) -> ParseResult<EdgePattern> {
    let mut variable = None;
    let mut label = None;
    let mut properties = None;

    let text = pair.as_str();
    let direction = if text.starts_with('<') && text.ends_with('>') {
        Direction::Both
    } else if text.starts_with('<') {
        Direction::Left
    } else if text.ends_with('>') {
        Direction::Right
    } else {
        Direction::Both
    };

    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::identifier => {
                variable = Some(inner_pair.as_str().to_string());
            }
            Rule::label => {
                label = Some(inner_pair.as_str().to_string());
            }
            Rule::properties => {
                properties = Some(build_properties(inner_pair)?);
            }
            _ => {}
        }
    }

    Ok(EdgePattern {
        variable,
        label,
        properties,
        direction,
    })
}

fn build_properties(pair: Pair<Rule>) -> ParseResult<HashMap<String, Expression>> {
    let mut map = HashMap::new();

    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::property {
            let (key, value) = build_property(inner_pair)?;
            map.insert(key, value);
        }
    }

    Ok(map)
}

fn build_property(pair: Pair<Rule>) -> ParseResult<(String, Expression)> {
    let mut inner = pair.into_inner();

    let key = inner
        .next()
        .ok_or_else(|| ParseError::InvalidSyntax("Missing property key".into()))?
        .as_str()
        .to_string();

    let value = build_expression(
        inner
            .next()
            .ok_or_else(|| ParseError::InvalidSyntax("Missing property value".into()))?,
    )?;

    Ok((key, value))
}

fn build_expression(pair: Pair<Rule>) -> ParseResult<Expression> {
    match pair.as_rule() {
        Rule::expression | Rule::or_expression | Rule::and_expression | Rule::not_expression
        | Rule::comparison_expression | Rule::additive_expression
        | Rule::multiplicative_expression | Rule::unary_expression | Rule::postfix_expression
        | Rule::primary_expression => {
            if pair.as_str().is_empty() {
                return Err(ParseError::InvalidSyntax("Empty expression".into()));
            }

            // For now, simplified expression parsing
            // Full implementation would handle all operators properly
            let inner_pairs: Vec<_> = pair.clone().into_inner().collect();

            if inner_pairs.is_empty() {
                // Terminal expression
                return build_terminal_expression(pair);
            }

            // Handle operators
            if inner_pairs.len() == 1 {
                return build_expression(inner_pairs[0].clone());
            }

            // Handle postfix expression with property lookups: identifier.property.property...
            if pair.as_rule() == Rule::postfix_expression {
                // Check if we have property lookups
                let has_property_lookups = inner_pairs.iter().any(|p| p.as_rule() == Rule::property_lookup);
                if has_property_lookups {
                    // Build property expression from identifier and property lookups
                    let mut base: Option<String> = None;
                    let mut properties: Vec<String> = Vec::new();

                    for inner_pair in &inner_pairs {
                        match inner_pair.as_rule() {
                            Rule::primary_expression => {
                                // Get the identifier from primary_expression
                                let primary_inner = inner_pair.clone().into_inner().next();
                                if let Some(id_pair) = primary_inner {
                                    if id_pair.as_rule() == Rule::identifier {
                                        base = Some(id_pair.as_str().to_string());
                                    }
                                }
                            }
                            Rule::property_lookup => {
                                // Get property name from property_lookup (skip the dot)
                                if let Some(id_pair) = inner_pair.clone().into_inner().next() {
                                    if id_pair.as_rule() == Rule::identifier {
                                        properties.push(id_pair.as_str().to_string());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(base_name) = base {
                        if !properties.is_empty() {
                            return Ok(Expression::Property(PropertyExpression {
                                base: base_name,
                                properties,
                            }));
                        }
                    }
                }
            }

            // Binary expression
            if inner_pairs.len() >= 3 {
                let left = build_expression(inner_pairs[0].clone())?;
                let op_pair = &inner_pairs[1];
                let right = build_expression(inner_pairs[2].clone())?;

                let op = match op_pair.as_rule() {
                    Rule::comparison_op => match op_pair.as_str() {
                        "=" => BinaryOperator::Eq,
                        "<>" | "!=" => BinaryOperator::Neq,
                        "<" => BinaryOperator::Lt,
                        ">" => BinaryOperator::Gt,
                        "<=" => BinaryOperator::Lte,
                        ">=" => BinaryOperator::Gte,
                        _ => {
                            return Err(ParseError::InvalidSyntax(format!(
                                "Unknown comparison op: {}",
                                op_pair.as_str()
                            )))
                        }
                    },
                    _ => {
                        // Handle other operators
                        match op_pair.as_str().to_uppercase().as_str() {
                            "AND" => BinaryOperator::And,
                            "OR" => BinaryOperator::Or,
                            "+" => BinaryOperator::Add,
                            "-" => BinaryOperator::Subtract,
                            "*" => BinaryOperator::Multiply,
                            "/" => BinaryOperator::Divide,
                            "%" => BinaryOperator::Modulo,
                            _ => {
                                return Err(ParseError::InvalidSyntax(format!(
                                    "Unknown operator: {}",
                                    op_pair.as_str()
                                )))
                            }
                        }
                    }
                };

                return Ok(Expression::BinaryOp {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                });
            }

            // Fallback
            build_terminal_expression(pair)
        }
        _ => build_terminal_expression(pair),
    }
}

fn build_terminal_expression(pair: Pair<Rule>) -> ParseResult<Expression> {
    match pair.as_rule() {
        Rule::literal => build_literal(pair),
        Rule::identifier => Ok(Expression::Variable(pair.as_str().to_string())),
        Rule::parameter => Ok(Expression::Parameter(
            pair.as_str().trim_start_matches('$').to_string(),
        )),
        Rule::function_call => build_function_call(pair),
        Rule::property_expression => Ok(Expression::Property(build_property_expression(pair)?)),
        _ => {
            // Try to recurse into inner pairs
            let rule = pair.as_rule();
            let mut inner = pair.into_inner();
            if let Some(inner_pair) = inner.next() {
                build_expression(inner_pair)
            } else {
                Err(ParseError::InvalidSyntax(format!(
                    "Cannot build expression from rule: {:?}",
                    rule
                )))
            }
        }
    }
}

fn build_literal(pair: Pair<Rule>) -> ParseResult<Expression> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ParseError::InvalidSyntax("Empty literal".into()))?;

    let lit = match inner.as_rule() {
        Rule::null => Literal::Null,
        Rule::boolean => Literal::Boolean(inner.as_str().to_uppercase() == "TRUE"),
        Rule::number => {
            let num_str = inner.as_str();
            if num_str.contains('.') || num_str.contains('e') || num_str.contains('E') {
                Literal::Float(
                    num_str
                        .parse()
                        .map_err(|_| ParseError::InvalidSyntax("Invalid float".into()))?,
                )
            } else {
                Literal::Integer(
                    num_str
                        .parse()
                        .map_err(|_| ParseError::InvalidSyntax("Invalid integer".into()))?,
                )
            }
        }
        Rule::string => {
            let s = inner.as_str();
            // Remove quotes
            let content = &s[1..s.len() - 1];
            Literal::String(content.to_string())
        }
        Rule::list_literal => {
            let mut items = Vec::new();
            for item_pair in inner.into_inner() {
                if item_pair.as_rule() == Rule::expression {
                    items.push(build_expression(item_pair)?);
                }
            }
            Literal::List(items)
        }
        Rule::map_literal => {
            let mut map = HashMap::new();
            for entry_pair in inner.into_inner() {
                if entry_pair.as_rule() == Rule::map_entry {
                    let (k, v) = build_map_entry(entry_pair)?;
                    map.insert(k, v);
                }
            }
            Literal::Map(map)
        }
        _ => {
            return Err(ParseError::InvalidSyntax(format!(
                "Unknown literal type: {:?}",
                inner.as_rule()
            )))
        }
    };

    Ok(Expression::Literal(lit))
}

fn build_map_entry(pair: Pair<Rule>) -> ParseResult<(String, Expression)> {
    let mut inner = pair.into_inner();

    let key = inner
        .next()
        .ok_or_else(|| ParseError::InvalidSyntax("Missing map key".into()))?
        .as_str()
        .to_string();

    let value = build_expression(
        inner
            .next()
            .ok_or_else(|| ParseError::InvalidSyntax("Missing map value".into()))?,
    )?;

    Ok((key, value))
}

fn build_function_call(pair: Pair<Rule>) -> ParseResult<Expression> {
    let mut inner = pair.into_inner();

    let name = inner
        .next()
        .ok_or_else(|| ParseError::InvalidSyntax("Missing function name".into()))?
        .as_str()
        .to_string();

    let mut args = Vec::new();
    for arg_pair in inner {
        if arg_pair.as_rule() == Rule::expression {
            args.push(build_expression(arg_pair)?);
        }
    }

    Ok(Expression::FunctionCall { name, args })
}

fn build_property_expression(pair: Pair<Rule>) -> ParseResult<PropertyExpression> {
    let mut parts: Vec<String> = Vec::new();

    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::identifier {
            parts.push(inner_pair.as_str().to_string());
        }
    }

    if parts.is_empty() {
        return Err(ParseError::InvalidSyntax(
            "Empty property expression".into(),
        ));
    }

    let base = parts.remove(0);
    let properties = parts;

    Ok(PropertyExpression { base, properties })
}
