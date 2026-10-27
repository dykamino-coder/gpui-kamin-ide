use crate::common::{is_name_char, is_name_start_char, is_whitespace_char};
use crate::reader::error::SyntaxError;
use crate::reader::lexer::Token;

use super::{DoctypeSubstate, PullParser, QuoteToken, Result, State};

impl PullParser {
    pub fn inside_doctype(&mut self, t: Token, substate: DoctypeSubstate) -> Option<Result> {
        if let Some(ref mut doctype) = self.data.doctype {
            admit!(t.push_to_text(doctype, &self.control));
            if !self.control.bounded() && doctype.len() > self.config.max_data_length {
                return Some(self.error(|| SyntaxError::ExceededConfiguredLimit));
            }
        }

        match substate {
            DoctypeSubstate::Outside => match t {
                Token::TagEnd => self.into_state_continue(State::OutsideTag),
                Token::MarkupDeclarationStart => {
                    self.buf.clear();
                    self.into_state_continue(State::InsideDoctype(DoctypeSubstate::InsideName))
                }
                Token::Character('%') => {
                    self.data.ref_data.clear();
                    admit!(self.data.ref_data.push('%', &self.control));
                    self.into_state_continue(State::InsideDoctype(
                        DoctypeSubstate::PEReferenceInDtd,
                    ))
                }
                Token::CommentStart => {
                    self.into_state_continue(State::InsideDoctype(DoctypeSubstate::Comment))
                }
                Token::SingleQuote | Token::DoubleQuote => {
                    // just discard string literals
                    self.data.quote = super::QuoteToken::from_token(t);
                    self.into_state_continue(State::InsideDoctype(DoctypeSubstate::String))
                }
                Token::CDataEnd | Token::CDataStart => {
                    Some(self.error(|| SyntaxError::UnexpectedToken(t)))
                }
                // TODO: parse SYSTEM, and [
                _ => None,
            },
            DoctypeSubstate::String => match t {
                Token::SingleQuote if self.data.quote != Some(QuoteToken::SingleQuoteToken) => None,
                Token::DoubleQuote if self.data.quote != Some(QuoteToken::DoubleQuoteToken) => None,
                Token::SingleQuote | Token::DoubleQuote => {
                    self.data.quote = None;
                    self.into_state_continue(State::InsideDoctype(DoctypeSubstate::Outside))
                }
                _ => None,
            },
            DoctypeSubstate::Comment => match t {
                Token::CommentEnd => {
                    self.into_state_continue(State::InsideDoctype(DoctypeSubstate::Outside))
                }
                _ => None,
            },
            DoctypeSubstate::InsideName => match t {
                Token::Character(c @ 'A'..='Z') => {
                    admit!(self.buf.push(c, &self.control));
                    None
                }
                Token::Character(c) if is_whitespace_char(c) => {
                    let buf = self.take_buf();
                    match buf.as_str() {
                        "ENTITY" => self.into_state_continue(State::InsideDoctype(
                            DoctypeSubstate::BeforeEntityName,
                        )),
                        "NOTATION" | "ELEMENT" | "ATTLIST" => self.into_state_continue(
                            State::InsideDoctype(DoctypeSubstate::SkipDeclaration),
                        ),
                        _ => Some(self.error(|| SyntaxError::UnknownMarkupDeclaration(buf.into()))),
                    }
                }
                _ => Some(self.error(|| SyntaxError::UnexpectedToken(t))),
            },
            DoctypeSubstate::BeforeEntityName => {
                self.data.name.clear();
                match t {
                    Token::Character(c) if is_whitespace_char(c) => None,
                    Token::Character('%') => {
                        // % is for PEDecl
                        admit!(self.data.name.push('%', &self.control));
                        self.into_state_continue(State::InsideDoctype(
                            DoctypeSubstate::PEReferenceDefinitionStart,
                        ))
                    }
                    Token::Character(c) if is_name_start_char(c) => {
                        if !self.control.bounded()
                            && self.data.name.len() > self.config.max_name_length
                        {
                            return Some(self.error(|| SyntaxError::ExceededConfiguredLimit));
                        }
                        admit!(self.data.name.push(c, &self.control));
                        self.into_state_continue(State::InsideDoctype(DoctypeSubstate::EntityName))
                    }
                    _ => Some(self.error(|| SyntaxError::UnexpectedTokenInEntity(t))),
                }
            }
            DoctypeSubstate::EntityName => match t {
                Token::Character(c) if is_whitespace_char(c) => self
                    .into_state_continue(State::InsideDoctype(DoctypeSubstate::BeforeEntityValue)),
                Token::Character(c) if is_name_char(c) => {
                    if !self.control.bounded() && self.data.name.len() > self.config.max_name_length
                    {
                        return Some(self.error(|| SyntaxError::ExceededConfiguredLimit));
                    }
                    admit!(self.data.name.push(c, &self.control));
                    None
                }
                _ => Some(self.error(|| SyntaxError::UnexpectedTokenInEntity(t))),
            },
            DoctypeSubstate::BeforeEntityValue => {
                self.buf.clear();
                match t {
                    Token::Character(c) if is_whitespace_char(c) => None,
                    // SYSTEM/PUBLIC not supported
                    Token::Character('S' | 'P') => {
                        let name = self.data.take_name();
                        admit!(self
                            .entities
                            .insert_first(name, super::Text::new(), &self.control)); // Dummy value, but at least the name is recognized

                        self.into_state_continue(State::InsideDoctype(
                            DoctypeSubstate::SkipDeclaration,
                        ))
                    }
                    Token::SingleQuote | Token::DoubleQuote => {
                        self.data.quote = super::QuoteToken::from_token(t);
                        self.into_state_continue(State::InsideDoctype(DoctypeSubstate::EntityValue))
                    }
                    _ => Some(self.error(|| SyntaxError::UnexpectedTokenInEntity(t))),
                }
            }
            DoctypeSubstate::EntityValue => match t {
                Token::SingleQuote if self.data.quote != Some(QuoteToken::SingleQuoteToken) => {
                    admit!(self.buf.push('\'', &self.control));
                    None
                }
                Token::DoubleQuote if self.data.quote != Some(QuoteToken::DoubleQuoteToken) => {
                    admit!(self.buf.push('"', &self.control));
                    None
                }
                Token::SingleQuote | Token::DoubleQuote => {
                    self.data.quote = None;
                    let name = self.data.take_name();
                    let val = self.take_buf();
                    admit!(self.entities.insert_first(name, val, &self.control)); // First wins
                    self.into_state_continue(State::InsideDoctype(DoctypeSubstate::SkipDeclaration))
                    // FIXME
                }
                Token::ReferenceStart | Token::Character('&') => {
                    self.data.ref_data.clear();
                    self.into_state_continue(State::InsideDoctype(
                        DoctypeSubstate::NumericReferenceStart,
                    ))
                }
                Token::Character('%') => {
                    self.data.ref_data.clear();
                    admit!(self.data.ref_data.push('%', &self.control)); // include literal % in the name to distinguish from regular entities
                    self.into_state_continue(State::InsideDoctype(
                        DoctypeSubstate::PEReferenceInValue,
                    ))
                }
                Token::Character(c) if !self.is_valid_xml_char(c) => {
                    Some(self.error(|| SyntaxError::InvalidCharacterEntity(c as u32)))
                }
                Token::Character(c) => {
                    admit!(self.buf.push(c, &self.control));
                    None
                }
                _ => Some(self.error(|| SyntaxError::UnexpectedTokenInEntity(t))),
            },
            DoctypeSubstate::PEReferenceDefinitionStart => match t {
                Token::Character(c) if is_whitespace_char(c) => None,
                Token::Character(c) if is_name_start_char(c) => {
                    debug_assert_eq!(self.data.name, "%");
                    admit!(self.data.name.push(c, &self.control));
                    self.into_state_continue(State::InsideDoctype(
                        DoctypeSubstate::PEReferenceDefinition,
                    ))
                }
                _ => Some(self.error(|| SyntaxError::UnexpectedTokenInEntity(t))),
            },
            DoctypeSubstate::PEReferenceDefinition => match t {
                Token::Character(c) if is_name_char(c) => {
                    if !self.control.bounded() && self.data.name.len() > self.config.max_name_length
                    {
                        return Some(self.error(|| SyntaxError::ExceededConfiguredLimit));
                    }
                    admit!(self.data.name.push(c, &self.control));
                    None
                }
                Token::Character(c) if is_whitespace_char(c) => self
                    .into_state_continue(State::InsideDoctype(DoctypeSubstate::BeforeEntityValue)),
                _ => Some(self.error(|| SyntaxError::UnexpectedTokenInEntity(t))),
            },
            DoctypeSubstate::PEReferenceInDtd => match t {
                Token::Character(c) if is_name_char(c) => {
                    admit!(self.data.ref_data.push(c, &self.control));
                    None
                }
                Token::ReferenceEnd | Token::Character(';') => {
                    let name = self.data.take_ref_data();
                    match admit!(self.entities.get(&name, &self.control)) {
                        Some(ent) => {
                            if let Err(e) = self.lexer.reparse_entity(Some(&name), ent) {
                                return Some(Err(e));
                            }
                            self.into_state_continue(State::InsideDoctype(DoctypeSubstate::Outside))
                        }
                        None => Some(self.error(|| SyntaxError::UndefinedEntity(name.into()))),
                    }
                }
                _ => Some(self.error(|| SyntaxError::UnexpectedTokenInEntity(t))),
            },
            DoctypeSubstate::PEReferenceInValue => match t {
                Token::Character(c) if is_name_char(c) => {
                    admit!(self.data.ref_data.push(c, &self.control));
                    None
                }
                Token::ReferenceEnd | Token::Character(';') => {
                    let name = self.data.take_ref_data();
                    match admit!(self.entities.get(&name, &self.control)) {
                        Some(ent) => {
                            admit!(self.buf.push_str(ent, &self.control));
                            self.into_state_continue(State::InsideDoctype(
                                DoctypeSubstate::EntityValue,
                            ))
                        }
                        None => Some(self.error(|| SyntaxError::UndefinedEntity(name.into()))),
                    }
                }
                _ => Some(self.error(|| SyntaxError::UnexpectedTokenInEntity(t))),
            },
            DoctypeSubstate::NumericReferenceStart => match t {
                Token::Character('#') => self
                    .into_state_continue(State::InsideDoctype(DoctypeSubstate::NumericReference)),
                Token::Character(c) if !self.is_valid_xml_char(c) => {
                    Some(self.error(|| SyntaxError::InvalidCharacterEntity(c as u32)))
                }
                Token::Character(c) => {
                    admit!(self.buf.push('&', &self.control));
                    admit!(self.buf.push(c, &self.control));
                    // named entities are not expanded inside doctype
                    self.into_state_continue(State::InsideDoctype(DoctypeSubstate::EntityValue))
                }
                _ => Some(self.error(|| SyntaxError::UnexpectedTokenInEntity(t))),
            },
            DoctypeSubstate::NumericReference => match t {
                Token::ReferenceEnd | Token::Character(';') => {
                    let r = self.data.take_ref_data();
                    // https://www.w3.org/TR/xml/#sec-entexpand
                    match self.numeric_reference_from_str(&r) {
                        Ok(c) => {
                            admit!(self.buf.push(c, &self.control));
                            self.into_state_continue(State::InsideDoctype(
                                DoctypeSubstate::EntityValue,
                            ))
                        }
                        Err(e) => Some(Err(e)),
                    }
                }
                Token::Character(c) if !self.is_valid_xml_char(c) => {
                    Some(self.error(|| SyntaxError::InvalidCharacterEntity(c as u32)))
                }
                Token::Character(c) => {
                    admit!(self.data.ref_data.push(c, &self.control));
                    None
                }
                _ => Some(self.error(|| SyntaxError::UnexpectedTokenInEntity(t))),
            },
            DoctypeSubstate::SkipDeclaration => match t {
                Token::TagEnd => {
                    self.into_state_continue(State::InsideDoctype(DoctypeSubstate::Outside))
                }
                _ => None,
            },
        }
    }
}
