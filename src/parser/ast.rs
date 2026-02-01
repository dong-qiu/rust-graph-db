/// Abstract Syntax Tree (AST) for Cypher queries
///
/// This module defines the AST nodes for representing parsed Cypher queries.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level Cypher query
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CypherQuery {
    /// Read query: MATCH ... WHERE ... RETURN ...
    Read {
        match_clause: MatchClause,
        where_clause: Option<WhereClause>,
        return_clause: ReturnClause,
    },

    /// Write query: CREATE/DELETE/SET
    Write(WriteClause),

    /// Mixed query: MATCH ... CREATE/DELETE/SET ...
    Mixed {
        match_clause: MatchClause,
        where_clause: Option<WhereClause>,
        write_clause: WriteClause,
        return_clause: Option<ReturnClause>,
    },
}

/// MATCH clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchClause {
    pub patterns: Vec<Pattern>,
}

/// WHERE clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhereClause {
    pub condition: Expression,
}

/// RETURN clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnClause {
    pub items: Vec<ReturnItem>,
    pub order_by: Option<Vec<SortItem>>,
    pub limit: Option<i64>,
}

/// RETURN item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnItem {
    pub expression: Expression,
    pub alias: Option<String>,
}

/// Sort item (ORDER BY)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortItem {
    pub expression: Expression,
    pub ascending: bool,
}

/// Write clauses (CREATE/DELETE/SET)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WriteClause {
    Create { patterns: Vec<Pattern> },
    Delete { expressions: Vec<Expression>, detach: bool },
    Set { items: Vec<SetItem> },
}

/// SET item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetItem {
    pub property: PropertyExpression,
    pub value: Expression,
}

/// Pattern: (node)-[edge]->(node)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub elements: Vec<PatternElement>,
}

/// Pattern element (node or edge)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatternElement {
    Node(NodePattern),
    Edge(EdgePattern),
}

/// Node pattern: (variable:Label {properties})
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodePattern {
    pub variable: Option<String>,
    pub label: Option<String>,
    pub properties: Option<HashMap<String, Expression>>,
}

/// Edge pattern: -[variable:Label {properties}]->
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgePattern {
    pub variable: Option<String>,
    pub label: Option<String>,
    pub properties: Option<HashMap<String, Expression>>,
    pub direction: Direction,
}

/// Edge direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Left to right: ->
    Right,
    /// Right to left: <-
    Left,
    /// Undirected: -
    Both,
}

/// Expression
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    /// Literal value
    Literal(Literal),

    /// Variable reference
    Variable(String),

    /// Parameter: $name
    Parameter(String),

    /// Binary operation: left op right
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },

    /// Unary operation: op expr
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expression>,
    },

    /// Property access: expr.property
    Property(PropertyExpression),

    /// Array/list index: expr[index]
    Index {
        expr: Box<Expression>,
        index: Box<Expression>,
    },

    /// Function call: func(args)
    FunctionCall {
        name: String,
        args: Vec<Expression>,
    },
}

/// Property expression: a.b.c
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyExpression {
    pub base: String,
    pub properties: Vec<String>,
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOperator {
    // Arithmetic
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,

    // Comparison
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,

    // Logical
    And,
    Or,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,
    Minus,
    Plus,
}

/// Literal values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<Expression>),
    Map(HashMap<String, Expression>),
}

impl CypherQuery {
    /// Check if this is a read-only query
    pub fn is_read_only(&self) -> bool {
        matches!(self, CypherQuery::Read { .. })
    }

    /// Check if this query has a RETURN clause
    pub fn has_return(&self) -> bool {
        match self {
            CypherQuery::Read { .. } => true,
            CypherQuery::Mixed { return_clause, .. } => return_clause.is_some(),
            CypherQuery::Write(_) => false,
        }
    }
}

impl Pattern {
    /// Create a simple node-only pattern
    pub fn node(node: NodePattern) -> Self {
        Self {
            elements: vec![PatternElement::Node(node)],
        }
    }

    /// Create a pattern with nodes and edges
    pub fn with_edges(elements: Vec<PatternElement>) -> Self {
        Self { elements }
    }

    /// Get all node patterns
    pub fn nodes(&self) -> Vec<&NodePattern> {
        self.elements
            .iter()
            .filter_map(|e| match e {
                PatternElement::Node(n) => Some(n),
                _ => None,
            })
            .collect()
    }

    /// Get all edge patterns
    pub fn edges(&self) -> Vec<&EdgePattern> {
        self.elements
            .iter()
            .filter_map(|e| match e {
                PatternElement::Edge(e) => Some(e),
                _ => None,
            })
            .collect()
    }
}

impl NodePattern {
    /// Create a simple node pattern with just a variable
    pub fn simple(variable: impl Into<String>) -> Self {
        Self {
            variable: Some(variable.into()),
            label: None,
            properties: None,
        }
    }

    /// Create a node pattern with label
    pub fn with_label(variable: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            variable: Some(variable.into()),
            label: Some(label.into()),
            properties: None,
        }
    }
}

impl EdgePattern {
    /// Create a simple edge pattern
    pub fn simple(variable: impl Into<String>, direction: Direction) -> Self {
        Self {
            variable: Some(variable.into()),
            label: None,
            properties: None,
            direction,
        }
    }

    /// Create an edge pattern with label
    pub fn with_label(
        variable: impl Into<String>,
        label: impl Into<String>,
        direction: Direction,
    ) -> Self {
        Self {
            variable: Some(variable.into()),
            label: Some(label.into()),
            properties: None,
            direction,
        }
    }
}

impl Expression {
    /// Create a variable reference
    pub fn var(name: impl Into<String>) -> Self {
        Expression::Variable(name.into())
    }

    /// Create an integer literal
    pub fn int(value: i64) -> Self {
        Expression::Literal(Literal::Integer(value))
    }

    /// Create a string literal
    pub fn string(value: impl Into<String>) -> Self {
        Expression::Literal(Literal::String(value.into()))
    }

    /// Create a boolean literal
    pub fn bool(value: bool) -> Self {
        Expression::Literal(Literal::Boolean(value))
    }

    /// Create NULL
    pub fn null() -> Self {
        Expression::Literal(Literal::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_creation() {
        let node = NodePattern::simple("n");
        let pattern = Pattern::node(node);

        assert_eq!(pattern.nodes().len(), 1);
        assert_eq!(pattern.edges().len(), 0);
    }

    #[test]
    fn test_expression_helpers() {
        let var = Expression::var("x");
        assert!(matches!(var, Expression::Variable(_)));

        let int = Expression::int(42);
        assert!(matches!(int, Expression::Literal(Literal::Integer(42))));

        let str_expr = Expression::string("hello");
        assert!(matches!(str_expr, Expression::Literal(Literal::String(_))));
    }

    #[test]
    fn test_query_types() {
        let read_query = CypherQuery::Read {
            match_clause: MatchClause { patterns: vec![] },
            where_clause: None,
            return_clause: ReturnClause {
                items: vec![],
                order_by: None,
                limit: None,
            },
        };

        assert!(read_query.is_read_only());
        assert!(read_query.has_return());

        let write_query = CypherQuery::Write(WriteClause::Create { patterns: vec![] });
        assert!(!write_query.is_read_only());
        assert!(!write_query.has_return());
    }
}
