/// Cypher query parser
///
/// This module provides a parser for Cypher queries using the pest library.

pub mod ast;
mod builder;

use ast::*;
use pest::Parser;
use pest_derive::Parser;
use thiserror::Error;

#[derive(Parser)]
#[grammar = "parser/cypher.pest"]
pub struct CypherParser;

/// Parser errors
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Pest parsing error: {0}")]
    PestError(#[from] Box<pest::error::Error<Rule>>),

    #[error("Invalid syntax: {0}")]
    InvalidSyntax(String),

    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),
}

pub type ParseResult<T> = Result<T, ParseError>;

/// Parse a Cypher query string into an AST
///
/// # Arguments
/// * `input` - The Cypher query string
///
/// # Returns
/// * `Ok(CypherQuery)` - Parsed AST
/// * `Err(ParseError)` - Parse error
///
/// # Examples
/// ```
/// use rust_graph_db::parser::parse_cypher;
///
/// let query = "MATCH (n:Person) RETURN n";
/// let ast = parse_cypher(query).unwrap();
/// ```
pub fn parse_cypher(input: &str) -> ParseResult<CypherQuery> {
    let pairs = CypherParser::parse(Rule::cypher_query, input)
        .map_err(|e| ParseError::PestError(Box::new(e)))?;

    builder::build_ast(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_match() {
        let query = "MATCH (n) RETURN n;";
        let result = parse_cypher(query);
        assert!(result.is_ok());

        let ast = result.unwrap();
        assert!(ast.has_return());
    }

    #[test]
    fn test_parse_match_with_label() {
        let query = "MATCH (n:Person) RETURN n;";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_match_with_properties() {
        let query = "MATCH (n:Person {name: 'Alice'}) RETURN n;";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_create() {
        let query = "CREATE (n:Person {name: 'Bob'});";
        let result = parse_cypher(query);
        assert!(result.is_ok());

        let ast = result.unwrap();
        assert!(!ast.is_read_only());
    }

    #[test]
    fn test_parse_match_edge() {
        let query = "MATCH (a)-[r:KNOWS]->(b) RETURN a, r, b;";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_delete() {
        let query = "MATCH (n:Person) DELETE n;";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_set() {
        let query = "MATCH (n:Person) SET n.age = 30;";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_set_nested_property() {
        let query = "MATCH (n:Person) SET n.address.city = 'Shanghai';";
        let result = parse_cypher(query);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

        if let Ok(CypherQuery::Mixed { write_clause: WriteClause::Set { items }, .. }) = result {
            assert_eq!(items.len(), 1);
            let item = &items[0];
            assert_eq!(item.property.base, "n");
            assert_eq!(item.property.properties, vec!["address", "city"]);
        } else {
            panic!("Expected Mixed query with SET clause");
        }
    }

    #[test]
    fn test_parse_set_deep_nested_property() {
        let query = "MATCH (n:Person) SET n.contact.address.city = 'Beijing';";
        let result = parse_cypher(query);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

        if let Ok(CypherQuery::Mixed { write_clause: WriteClause::Set { items }, .. }) = result {
            assert_eq!(items.len(), 1);
            let item = &items[0];
            assert_eq!(item.property.base, "n");
            assert_eq!(item.property.properties, vec!["contact", "address", "city"]);
        } else {
            panic!("Expected Mixed query with SET clause");
        }
    }

    #[test]
    fn test_parse_with_clause() {
        let query = "MATCH (p:Person) WITH p, p.age AS age RETURN p.name, age;";
        let result = parse_cypher(query);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

        if let Ok(CypherQuery::WithQuery {
            with_clause,
            return_clause,
            ..
        }) = result
        {
            assert_eq!(with_clause.items.len(), 2);
            assert_eq!(return_clause.items.len(), 2);
        } else {
            panic!("Expected WithQuery");
        }
    }

    #[test]
    fn test_parse_with_where() {
        let query = "MATCH (p:Person) WITH p, p.age AS age WHERE age > 25 RETURN p.name;";
        let result = parse_cypher(query);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

        if let Ok(CypherQuery::WithQuery { with_where, .. }) = result {
            assert!(with_where.is_some());
        } else {
            panic!("Expected WithQuery with WHERE after WITH");
        }
    }

    #[test]
    fn test_parse_with_order_limit() {
        let query = "MATCH (p:Person) WITH p, p.age AS age ORDER BY age LIMIT 10 RETURN p;";
        let result = parse_cypher(query);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

        if let Ok(CypherQuery::WithQuery { with_clause, .. }) = result {
            assert!(with_clause.order_by.is_some());
            assert_eq!(with_clause.limit, Some(10));
        } else {
            panic!("Expected WithQuery");
        }
    }

    #[test]
    fn test_parse_invalid_query() {
        let query = "INVALID QUERY";
        let result = parse_cypher(query);
        assert!(result.is_err());
    }
}
