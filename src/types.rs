//! Provides a type system for the AST, in some sense

use crate::{
    parser::{Node, NodeType, Types},
    tokenizer::Token,
    value::{self, Value as ParsedValue, ValueError}
};

use rowan::{SmolStr, WalkEvent};

fn cast_into<R: rowan::TreeRoot<Types>>(from: Node<R>, kind: NodeType) -> Option<Node<R>> {
    if from.kind() == kind {
        Some(from)
    } else {
        None
    }
}

macro_rules! typed {
    ($($kind:expr => $name:ident$(: $trait:ident)*$(: { $($block:tt)* })*),*) => {
        $(
            pub struct $name<R: rowan::TreeRoot<Types>>(Node<R>);

            impl<R: rowan::TreeRoot<Types>> TypedNode<R> for $name<R> {
                fn cast(from: Node<R>) -> Option<Self> {
                    cast_into(from, $kind.into()).map($name)
                }
                fn node(&self) -> &Node<R> {
                    &self.0
                }
                fn into_node(self) -> Node<R> {
                    self.0
                }
            }
            $(impl<R: rowan::TreeRoot<Types>> $trait<R> for $name<R> {})*
            $(impl<R: rowan::TreeRoot<Types>> $name<R> { $($block)* })*
        )*
    }
}
macro_rules! nth {
    ($self:expr; $index:expr) => {
        $self.children()
            .nth($index)
            .expect("invalid ast")
    };
    ($self:expr; ($kind:ident) $index:expr) => {
        $kind::cast(nth!($self; $index)).expect("invalid ast")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OpKind {
    Concat,
    IsSet,
    Merge,

    Add,
    Sub,
    Mul,
    Div,

    And,
    Equal,
    Implication,
    Less,
    LessOrEq,
    More,
    MoreOrEq,
    NotEqual,
    Or
}
impl OpKind {
    /// Get the operation kind from a token in the AST
    pub fn from_token(token: Token) -> Option<Self> {
        match token {
            Token::Concat => Some(OpKind::Concat),
            Token::Question => Some(OpKind::IsSet),
            Token::Merge => Some(OpKind::Merge),

            Token::Add => Some(OpKind::Add),
            Token::Sub => Some(OpKind::Sub),
            Token::Mul => Some(OpKind::Mul),
            Token::Div => Some(OpKind::Div),

            Token::And => Some(OpKind::And),
            Token::Equal => Some(OpKind::Equal),
            Token::Implication => Some(OpKind::Implication),
            Token::Less => Some(OpKind::Less),
            Token::LessOrEq => Some(OpKind::LessOrEq),
            Token::More => Some(OpKind::More),
            Token::MoreOrEq => Some(OpKind::MoreOrEq),
            Token::NotEqual => Some(OpKind::NotEqual),
            Token::Or => Some(OpKind::Or),

            _ => None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnaryOpKind {
    Invert,
    Negate
}
impl UnaryOpKind {
    /// Get the operation kind from a token in the AST
    pub fn from_token(token: Token) -> Option<Self> {
        match token {
            Token::Invert => Some(UnaryOpKind::Invert),
            Token::Sub => Some(UnaryOpKind::Negate),
            _ => None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InterpolPart<R: rowan::TreeRoot<Types>> {
    Literal(String),
    Ast(Node<R>)
}

/// A TypedNode is simply a wrapper around an untyped node to provide a type
/// system in some sense.
pub trait TypedNode<R: rowan::TreeRoot<Types>> where Self: Sized {
    /// Cast an untyped node into this strongly-typed node. This will return
    /// None if the type was not correct.
    fn cast(from: Node<R>) -> Option<Self>;
    /// Return a reference to the inner untyped node
    fn node(&self) -> &Node<R>;
    /// Return the inner untyped node
    fn into_node(self) -> Node<R>;
    /// Return all non-trivia (so no comments, whitespace, errors) children
    fn children<'a>(&'a self) -> Box<Iterator<Item = Node<R>> + 'a>
        where R: 'a
    {
        Box::new(
            self.node()
                .children()
                .filter(|node| !node.kind().is_trivia())
        )
    }
    /// Return all errors of all children, recursively
    fn errors<'a>(&'a self) -> Vec<Node<rowan::RefRoot<'a, Types>>>
        where R: 'a
    {
        let mut bucket = Vec::new();
        for event in self.node().borrowed().preorder() {
            if let WalkEvent::Enter(node) = event {
                match node.kind() {
                    NodeType::Error | NodeType::Token(Token::Error) => bucket.push(node),
                    _ => ()
                }
            }
        }
        bucket
    }
}
/// Provides the function `.entries()`
pub trait EntryHolder<R: rowan::TreeRoot<Types>>: TypedNode<R> {
    /// Return an iterator over all key=value entries
    fn entries<'a>(&'a self) -> Box<Iterator<Item = SetEntry<R>> + 'a>
        where R: 'a
    {
        Box::new(self.node().children().filter_map(SetEntry::cast))
    }
    /// Return an iterator over all inherit entries
    fn inherits<'a>(&'a self) -> Box<Iterator<Item = Inherit<R>> + 'a>
        where R: 'a
    {
        Box::new(self.node().children().filter_map(Inherit::cast))
    }
}
/// Provides the function `.inner()` for wrapping types like parenthensis
pub trait Wrapper<R: rowan::TreeRoot<Types>>: TypedNode<R> {
    /// Return the inner value
    fn inner(&self) -> Node<R> {
        nth!(self; 1)
    }
}
/// Provides the function `.inner()` for transparent wrappers like root
pub trait LightWrapper<R: rowan::TreeRoot<Types>>: TypedNode<R> {
    /// Return the inner value
    fn inner(&self) -> Node<R> {
        nth!(self; 0)
    }
}

pub struct Value<R: rowan::TreeRoot<Types>>(Node<R>);
impl<R: rowan::TreeRoot<Types>> TypedNode<R> for Value<R> {
    fn cast(from: Node<R>) -> Option<Self> {
        match from.kind() {
            NodeType::Token(t) if t.is_value() => Some(Value(from)),
            _ => None
        }
    }
    fn node(&self) -> &Node<R> {
        &self.0
    }
    fn into_node(self) -> Node<R> {
        self.0
    }
}
impl<R: rowan::TreeRoot<Types>> Value<R> {
    /// Return the value as a string
    pub fn as_str(&self) -> &str {
        self.node().borrowed().leaf_text().map(SmolStr::as_str).expect("invalid ast")
    }

    /// Parse the value
    pub fn to_value(&self) -> Result<ParsedValue, ValueError> {
        let token = match self.0.kind() {
            NodeType::Token(token) => token,
            _ => panic!("invalid value somehow constructed")
        };
        ParsedValue::from_token(token, self.as_str())
    }
}

typed! [
    Token::Ident => Ident: {
        /// Return the identifier as a string
        pub fn as_str(&self) -> &str {
            self.node().borrowed().leaf_text().map(SmolStr::as_str).expect("invalid ast")
        }
    },

    NodeType::Apply => Apply: {
        /// Return the lambda being applied
        pub fn lambda(&self) -> Node<R> {
            nth!(self; 0)
        }
        /// Return the value which the lambda is being applied with
        pub fn value(&self) -> Node<R> {
            nth!(self; 1)
        }
    },
    NodeType::Assert => Assert: {
        /// Return the assert condition
        pub fn condition(&self) -> Node<R> {
            nth!(self; 1)
        }
        /// Return the success body
        pub fn body(&self) -> Node<R> {
            nth!(self; 3)
        }
    },
    NodeType::Attribute => Attribute: {
        /// Return the path as an iterator of identifiers
        pub fn path<'a>(&'a self) -> impl Iterator<Item = Node<R>> + 'a {
            self.children().filter(|node| node.kind() != NodeType::Token(Token::Dot))
        }
    },
    NodeType::Dynamic => Dynamic: Wrapper,
    NodeType::Error => Error,
    NodeType::IfElse => IfElse: {
        /// Return the condition
        pub fn condition(&self) -> Node<R> {
            nth!(self; 1)
        }
        /// Return the success body
        pub fn body(&self) -> Node<R> {
            nth!(self; 3)
        }
        /// Return the else body
        pub fn else_body(&self) -> Node<R> {
            nth!(self; 5)
        }
    },
    NodeType::IndexSet => IndexSet: {
        /// Return the set being indexed
        pub fn set(&self) -> Node<R> {
            nth!(self; 0)
        }
        /// Return the index
        pub fn index(&self) -> Node<R> {
            nth!(self; 2)
        }
    },
    NodeType::Inherit => Inherit: {
        /// Return the set where keys are being inherited from, if any
        pub fn from(&self) -> Option<InheritFrom<R>> {
            self.node().children()
                .filter_map(InheritFrom::cast)
                .next()
        }
    },
    NodeType::InheritFrom => InheritFrom: Wrapper,
    NodeType::Interpol => Interpol: {
        /// Parse the interpolation into a series of parts
        pub fn parts(&self) -> Vec<InterpolPart<R>> {
            let mut parts = Vec::new();
            let mut literals = 0;
            let mut common = std::usize::MAX;
            let mut multiline = false;

            for child in self.node().children() {
                match child.kind() {
                    NodeType::InterpolAst => {
                        let ast = InterpolAst::cast(child).unwrap();
                        parts.push(InterpolPart::Ast(ast.inner()));
                    },
                    NodeType::InterpolLiteral => {
                        let literal = InterpolLiteral::cast(child).unwrap();
                        let token = literal.inner();
                        let mut text = token.borrowed().leaf_text().map(SmolStr::as_str).unwrap_or_default();

                        let start = token.kind() == NodeType::Token(Token::InterpolStart)
                            || token.kind() == NodeType::Token(Token::InterpolEndStart);
                        let end = token.kind() == NodeType::Token(Token::InterpolEnd)
                            || token.kind() == NodeType::Token(Token::InterpolEndStart);

                        match token.kind() {
                            NodeType::Token(Token::InterpolStart) => {
                                multiline = if text.starts_with('"') {
                                    text = &text[1..];
                                    false
                                } else if text.starts_with("''") {
                                    text = &text[2..];
                                    true
                                } else { false };
                            },
                            NodeType::Token(Token::InterpolEndStart) => (),
                            NodeType::Token(Token::InterpolEnd) => {
                                let len = text.len();
                                if text.ends_with('"') {
                                    text = &text[..len-1];
                                } else if text.ends_with("''") {
                                    text = &text[..len-2];
                                }
                            },
                            _ => continue
                        }
                        if end && text.starts_with("}") {
                            text = &text[1..];
                        }
                        if start && text.ends_with("${") {
                            let len = text.len();
                            text = &text[..len-2];
                        }
                        let line_count = text.lines().count();
                        for (i, line) in text.lines().enumerate().skip(if end { 1 } else { 0 }) {
                            let indent: usize = value::indention(line).count();
                            if (i != line_count-1 || !start) && indent == line.chars().count() {
                                // line is empty and not the start of an
                                // interpolation, ignore indention
                                continue;
                            }
                            common = common.min(indent);
                        }
                        parts.push(InterpolPart::Literal(text.to_string()));
                        literals += 1;
                    },
                    _ => ()
                }
            }

            let mut i = 0;
            for part in parts.iter_mut() {
                if let InterpolPart::Literal(ref mut text) = part {
                    if multiline {
                        *text = value::remove_indent(text, i == 0, common);
                        if i == literals-1 {
                            // Last index
                            value::remove_trailing(text);
                        }
                    }
                    *text = value::unescape(text, multiline);
                    i += 1;
                }
            }

            parts
        }
    },
    NodeType::InterpolAst => InterpolAst: LightWrapper,
    NodeType::InterpolLiteral => InterpolLiteral: LightWrapper,
    NodeType::Lambda => Lambda: {
        /// Return the argument of the lambda
        pub fn arg(&self) -> Node<R> {
            nth!(self; 0)
        }
        /// Return the body of the lambda
        pub fn body(&self) -> Node<R> {
            nth!(self; 2)
        }
    },
    NodeType::Let => Let: EntryHolder,
    NodeType::LetIn => LetIn: EntryHolder: {
        /// Return the body
        pub fn body(&self) -> Node<R> {
            self.children()
                .filter(|node| SetEntry::cast(node.borrowed()).is_none())
                .nth(2)
                .expect("invalid ast")
        }
    },
    NodeType::List => List: {
        /// Return an iterator over items in the list
        pub fn items(&self) -> impl Iterator<Item = ListItem<R>> {
            self.node().children().filter_map(ListItem::cast)
        }
    },
    NodeType::ListItem => ListItem: LightWrapper,
    NodeType::Operation => Operation: {
        /// Return the first value in the operation
        pub fn value1(&self) -> Node<R> {
            nth!(self; 0)
        }
        /// Return the operator
        pub fn operator(&self) -> OpKind {
            match nth!(self; 1).kind() {
                NodeType::Token(token) => OpKind::from_token(token).expect("invalid ast"),
                _ => panic!("invalid ast")
            }
        }
        /// Return the second value in the operation
        pub fn value2(&self) -> Node<R> {
            nth!(self; 2)
        }
    },
    NodeType::OrDefault => OrDefault: {
        /// Return the indexing operation
        pub fn index(&self) -> IndexSet<R> {
            nth!(self; (IndexSet) 0)
        }
        /// Return the default value
        pub fn default(&self) -> Node<R> {
            nth!(self; 2)
        }
    },
    NodeType::Paren => Paren: Wrapper,
    NodeType::PatBind => PatBind: {
        /// Return the identifier the set is being bound as
        pub fn name(&self) -> Ident<R> {
            self.node().children().filter_map(Ident::cast).next().expect("invalid ast")
        }
    },
    NodeType::PatEntry => PatEntry: {
        /// Return the identifier the argument is being bound as
        pub fn name(&self) -> Ident<R> {
            nth!(self; (Ident) 0)
        }
        /// Return the default value, if any
        pub fn default(&self) -> Option<Node<R>> {
            self.node().children()
                .filter(|node| node.kind().is_trivia())
                .nth(2)
        }
    },
    NodeType::Pattern => Pattern: {
        /// Return an iterator over all pattern entries
        pub fn entries(&self) -> impl Iterator<Item = PatEntry<R>> {
            self.node().children().filter_map(PatEntry::cast)
        }
        /// Returns true if this pattern is inexact (has an ellipsis, ...)
        pub fn ellipsis(&self) -> bool {
            self.node().children().any(|node| node.kind() == NodeType::Token(Token::Ellipsis))
        }
        /// Returns a clone of the tree root but without entries where the
        /// callback function returns false
        /// { a, b, c } without b is { a, c }
        pub fn filter_entries<F>(&self, mut callback: F) -> Root<rowan::OwnedRoot<Types>>
            where F: FnMut(PatEntry<rowan::RefRoot<Types>>) -> bool
        {
            // Filter entries
            let mut next_entry = None;
            let mut last_entry = true;
            let mut children: Vec<_> = self.node().borrowed().children()
                .filter(|node| {
                    if let Some(entry) = PatEntry::cast(*node) {
                        last_entry = match next_entry.take() {
                            Some(keep) => keep,
                            None => callback(entry)
                        };
                        last_entry
                    } else if node.kind() == NodeType::Token(Token::Comment) {
                        if let Some(keep) = next_entry {
                            keep
                        } else {
                            // Peek ahead
                            let mut next = Some(*node);
                            loop {
                                next = next.unwrap().next_sibling();
                                match next.map(|node| node.kind()) {
                                    Some(NodeType::Token(t)) if t.is_trivia() => (),
                                    Some(NodeType::PatEntry) => {
                                        let entry = PatEntry::cast(next.unwrap()).unwrap();
                                        last_entry = callback(entry);
                                        next_entry = Some(last_entry);
                                        break last_entry;
                                    },
                                    _ => break true
                                }
                            }
                        }
                    } else if node.kind() == NodeType::Token(Token::Comma) {
                        last_entry
                    } else if node.kind() == NodeType::Token(Token::Whitespace) {
                        last_entry
                    } else {
                        last_entry = true;
                        next_entry = None;
                        true
                    }
                })
                .map(|node| node.green().clone())
                .collect();

            // Remove trailing comma, if any
            let mut i = children.len();
            while i > 0 {
                i -= 1;
                match children.get(i).map(|node| node.kind()) {
                    Some(NodeType::Token(Token::CurlyBClose)) => (),
                    Some(NodeType::Token(t)) if t.is_trivia() => (),
                    _ => break
                }
            }
            if children.get(i).map(|node| node.kind()) == Some(NodeType::Token(Token::Comma)) {
                children.remove(i);
            }

            Root::cast(Node::new(
                self.node().with_children(children.into_boxed_slice()),
                Vec::new()
            )).unwrap()
        }
    },
    NodeType::Root => Root: LightWrapper,
    NodeType::Set => Set: EntryHolder: {
        /// Returns true if this set is recursive
        pub fn recursive(&self) -> bool {
            self.node().children().any(|node| node.kind() == NodeType::Token(Token::Rec))
        }
        /// Returns a clone of the tree root but without entries where the
        /// callback function returns false
        /// { a = 2; b = 3; } without 0 is { b = 3; }
        pub fn filter_entries<F>(&self, mut callback: F) -> Root<rowan::OwnedRoot<Types>>
            where F: FnMut(SetEntry<rowan::RefRoot<Types>>) -> bool
        {
            let mut next_entry = None;
            let mut last_entry = true;
            let children: Vec<_> = self.node().borrowed().children()
                .filter(|node| {
                    if let Some(entry) = SetEntry::cast(*node) {
                        if let Some(keep) = next_entry.take() {
                            keep
                        } else {
                            last_entry = callback(entry);
                            last_entry
                        }
                    } else if node.kind() == NodeType::Token(Token::Comment) {
                        if let Some(keep) = next_entry {
                            keep
                        } else {
                            // Peek ahead
                            let mut next = Some(*node);
                            loop {
                                next = next.unwrap().next_sibling();
                                match next.map(|node| node.kind()) {
                                    Some(NodeType::Token(t)) if t.is_trivia() => (),
                                    Some(NodeType::SetEntry) => {
                                        let entry = SetEntry::cast(next.unwrap()).unwrap();
                                        last_entry = callback(entry);
                                        next_entry = Some(last_entry);
                                        break last_entry;
                                    },
                                    _ => break true
                                }
                            }
                        }
                    } else if node.kind() == NodeType::Token(Token::Whitespace) {
                        last_entry
                    } else {
                        last_entry = true;
                        true
                    }
                })
                .map(|node| node.green().clone())
                .collect();

            Root::cast(Node::new(
                self.node().with_children(children.into_boxed_slice()),
                Vec::new()
            )).unwrap()
        }
    },
    NodeType::SetEntry => SetEntry: {
        /// Return this entry's key
        pub fn key(&self) -> Attribute<R> {
            nth!(self; (Attribute) 0)
        }
        /// Return this entry's value
        pub fn value(&self) -> Node<R> {
            nth!(self; 2)
        }
    },
    NodeType::Unary => Unary: {
        /// Return the operator
        pub fn operator(&self) -> UnaryOpKind {
            match nth!(self; 0).kind() {
                NodeType::Token(token) => UnaryOpKind::from_token(token).expect("invalid ast"),
                _ => panic!("invalid ast")
            }
        }
        /// Return the value in the operation
        pub fn value(&self) -> Node<R> {
            nth!(self; 1)
        }
    },
    NodeType::With => With: {
        /// Return the namespace
        pub fn namespace(&self) -> Node<R> {
            nth!(self; 1)
        }
        /// Return the body
        pub fn body(&self) -> Node<R> {
            nth!(self; 3)
        }
    }
];
