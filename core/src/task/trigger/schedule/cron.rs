use crate::errors::{CronError, CronErrorTypes, CronExpressionLexerErrors, CronExpressionParserErrors};
use crate::task::schedule::TaskSchedule;
use std::error::Error;
use std::ops::RangeInclusive;
use std::str::FromStr;
use std::time::SystemTime;

#[derive(Clone, Eq, PartialEq, Default)]
pub enum CronField {
    #[default]
    Wildcard,

    Exact(u8),
    Range(u8, u8),
    Step(Box<CronField>, u8),
    List(Vec<CronField>),
    Unspecified,
    Last(Option<i8>),
    NearestWeekday(u8),
    NthWeekday(u8, u8)
}

#[derive(Debug, PartialEq, Eq)]
enum TokenType {
    Value(u8),
    Minus,
    Wildcard,
    ListSeparator,
    Unspecified,
    Step,
    Last,
    NearestWeekday,
    NthWeekday,
}

#[derive(Debug)]
pub struct Token {
    start: usize,
    end: usize,
    token_type: TokenType
}

#[derive(Default, Clone, Debug)]
enum AstTreeNode {
    #[default]
    Wildcard,

    List(Vec<AstTreeNode>),
    Step(Box<AstTreeNode>, u8),
    Range(Box<AstTreeNode>, Box<AstTreeNode>),
    Exact(u8),
    LastOf(Option<u8>),
    Unspecified,
    NthWeekday(u8, u8),
    NearestWeekday(Box<AstTreeNode>),
}

const RANGES: [RangeInclusive<u8>; 6] = [
    0..=59u8,
    0..=59u8,
    0..=23u8,
    1u8..=31u8,
    1u8..=12u8,
    1u8..=7u8
];

struct CronParser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> CronParser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_field(&mut self) -> Result<AstTreeNode, CronExpressionParserErrors> {
        let node = self.parse_list()?;

        if !self.is_at_end() {
            return Err(CronExpressionParserErrors::UnexpectedToken);
        }

        Ok(node)
    }

    fn parse_list(&mut self) -> Result<AstTreeNode, CronExpressionParserErrors> {
        let mut segments = vec![self.parse_segment()?];

        while self.advanced_if(&TokenType::ListSeparator) {
            segments.push(self.parse_segment()?);
        }

        if segments.len() == 1 {
            return Ok(segments.remove(0));
        }

        Ok(AstTreeNode::List(segments))
    }

    fn parse_segment(&mut self) -> Result<AstTreeNode, CronExpressionParserErrors> {
        let base = self.parse_base()?;

        if self.advanced_if(&TokenType::Step) {
            let step = self.parse_atom()?;

            if let AstTreeNode::Exact(step) = step {
                return Ok(AstTreeNode::Step(
                    Box::new(base),
                    step
                ));
            }

            return Err(CronExpressionParserErrors::ExpectedNumber);
        } else if self.advanced_if(&TokenType::NthWeekday) {
            let AstTreeNode::Exact(val1) = base else {
                return Err(CronExpressionParserErrors::ExpectedNumber);
            };

            let AstTreeNode::Exact(val2) = self.parse_atom()? else {
                return Err(CronExpressionParserErrors::ExpectedNumber);
            };

            return Ok(AstTreeNode::NthWeekday(
                val1,
                val2
            ));

        } else if self.advanced_if(&TokenType::NearestWeekday) {
            if let AstTreeNode::Exact(val) = base {
                return Ok(AstTreeNode::NearestWeekday(Box::new(AstTreeNode::Exact(val))));
            } else if let AstTreeNode::LastOf(None) = base {
                return Ok(AstTreeNode::NearestWeekday(Box::new(AstTreeNode::LastOf(None))));
            }

            return Err(CronExpressionParserErrors::UnexpectedToken);
        } else if self.advanced_if(&TokenType::Last) {
            let AstTreeNode::Exact(val) = base else {
                return Err(CronExpressionParserErrors::ExpectedNumber);
            };

            return Ok(AstTreeNode::LastOf(Some(val)));
        }

        Ok(base)
    }

    fn parse_base(&mut self) -> Result<AstTreeNode, CronExpressionParserErrors> {
        let start = self.parse_atom()?;

        if self.advanced_if(&TokenType::Minus) {
            let end = self.parse_atom()?;
            return Ok(AstTreeNode::Range(
                Box::new(start),
                Box::new(end),
            ));
        }

        Ok(start)
    }

    fn parse_atom(&mut self) -> Result<AstTreeNode, CronExpressionParserErrors> {
        let token = self.peek().ok_or(CronExpressionParserErrors::UnexpectedEnd)?;

        match token.token_type {
            TokenType::Wildcard => {
                self.advance();
                Ok(AstTreeNode::Wildcard)
            }

            TokenType::Unspecified => {
                self.advance();
                Ok(AstTreeNode::Unspecified)
            }

            TokenType::Value(val) => {
                self.advance();
                Ok(AstTreeNode::Exact(val))
            }

            TokenType::Last => {
                self.advance();
                Ok(AstTreeNode::LastOf(None))
            }

            _ => Err(CronExpressionParserErrors::ExpectedAtom),
        }
    }

    fn advanced_if(&mut self, expected: &TokenType) -> bool {
        if self.check(expected) {
            self.advance();
            return true;
        }

        false
    }

    fn check(&self, expected: &TokenType) -> bool {
        if self.is_at_end() {
            return false;
        }

        self.peek().unwrap().token_type == *expected
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }
}

fn constant_to_numeric(
    char_buffer: &mut String,
    field_pos: usize,
    position: usize,
    tokens: &mut Vec<Token>
) -> Result<(), (CronExpressionLexerErrors, usize, usize)> {
    let num: u8;
    match &char_buffer[0..=2] {
        "SUN" | "sun" if field_pos == 3 => { num = 1; }
        "MON" | "mon" if field_pos == 3 => { num = 2; }
        "TUE" | "tue" if field_pos == 3 => { num = 3; }
        "WED" | "wed" if field_pos == 3 => { num = 4; }
        "THU" | "thu" if field_pos == 3 => { num = 5; }
        "FRI" | "fri" if field_pos == 3 => { num = 6; }
        "SAT" | "sat" if field_pos == 3 => { num = 7; }
        "JAN" | "jan" if field_pos == 5 => { num = 1; }
        "FEB" | "feb" if field_pos == 5 => { num = 2; }
        "MAR" | "mar" if field_pos == 5 => { num = 3; }
        "APR" | "apr" if field_pos == 5 => { num = 4; }
        "MAY" | "may" if field_pos == 5 => { num = 5; }
        "JUN" | "jun" if field_pos == 5 => { num = 6; }
        "JUL" | "jul" if field_pos == 5 => { num = 7; }
        "AUG" | "aug" if field_pos == 5 => { num = 8; }
        "SEP" | "sep" if field_pos == 5 => { num = 9; }
        "OCT" | "oct" if field_pos == 5 => { num = 10; }
        "NOV" | "nov" if field_pos == 5 => { num = 11; }
        "DEC" | "dec" if field_pos == 5 => { num = 12; }
        _ => {
            return Err((CronExpressionLexerErrors::UnknownCharacter, position, field_pos))
        }
    }

    tokens.push(Token {
        start: position - 3,
        end: position,
        token_type: TokenType::Value(num)
    });
    char_buffer.clear();
    Ok(())
}

fn try_allocate_number(
    position: usize,
    digit_start: &mut Option<usize>,
    current_number: &mut u8,
    tokens: &mut Vec<Token>
) {
    if let Some(start) = digit_start {
        tokens.push(Token {
            start: *start,
            end: position,
            token_type: TokenType::Value(*current_number)
        });
        *current_number = 0;
        *digit_start = None;
    }
}

fn tokenize_fields(s: &str) -> Result<[Vec<Token>; 6], (CronExpressionLexerErrors, usize, usize)> {
    let mut tokens: [Vec<Token>; 6] = [const { Vec::new() }; 6];
    let mut current_number = 0u8;
    let mut field_pos = 0;
    let mut char_buffer: String = String::with_capacity(3);
    let mut chars = s.chars().enumerate().peekable();
    let mut digit_start: Option<usize> = None;
    while let Some((position, char)) = chars.next()  {
        if char == ' ' {
            try_allocate_number(position, &mut digit_start, &mut current_number, &mut tokens[field_pos]);
            digit_start = None;
            current_number = 0;

            if char_buffer.len() == 3 {
                constant_to_numeric(&mut char_buffer, field_pos, position, &mut tokens[field_pos])?;
            } else if char_buffer.len() != 0 {
                return Err((CronExpressionLexerErrors::UnknownCharacter, position, field_pos))
            }

            if field_pos >= 5 {
                return Err((CronExpressionLexerErrors::UnknownFieldFormat, position, field_pos));
            }

            if tokens[field_pos].is_empty() && field_pos > 0 {
                return Err((CronExpressionLexerErrors::EmptyField, position, field_pos));
            }
            field_pos += 1;
            continue;
        }

        if char.is_alphabetic() || char_buffer.len() > 0 {
            char_buffer.push(char);
            if char_buffer.len() == 3 {
                constant_to_numeric(&mut char_buffer, field_pos, position, &mut tokens[field_pos])?;
                continue;
            }
        }

        if char.is_ascii_digit() {
            digit_start = Some(position);
            current_number = current_number * 10 + (char as u8 - b'0');
            continue;
        }

        try_allocate_number(position, &mut digit_start, &mut current_number, &mut tokens[field_pos]);

        let token_type = match char {
            '-' => TokenType::Minus,
            '*' => TokenType::Wildcard,
            ',' => TokenType::ListSeparator,
            '?' => TokenType::Unspecified,
            '/' => TokenType::Step,
            'L' => {
                char_buffer.clear();
                TokenType::Last
            },
            '#' => TokenType::NthWeekday,
            'W' if !matches!(chars.peek(), Some((_, 'E' | 'e'))) => {
                char_buffer.clear();
                TokenType::NearestWeekday
            },
            _ => return Err((CronExpressionLexerErrors::UnknownCharacter, position, field_pos)),
        };

        tokens[field_pos].push(Token {
            start: position,
            end: position,
            token_type
        })
    }

    if field_pos != 5 && field_pos != 4 {
        return Err((CronExpressionLexerErrors::UnknownFieldFormat, s.len() - 1, field_pos));
    }

    if !char_buffer.is_empty() {
        let position = s.len() - char_buffer.len();
        return Err((CronExpressionLexerErrors::UnknownCharacter, position, field_pos));
    }

    if let Some(start) = digit_start {
        tokens[field_pos].push(Token {
            start,
            end: s.len() - 1,
            token_type: TokenType::Value(current_number)
        });
    }

    Ok(tokens)
}

#[derive(Clone, Eq, PartialEq)]
pub struct TaskScheduleCron {
    seconds: CronField,
    minute: CronField,
    hour: CronField,
    day_of_month: CronField,
    month: CronField,
    day_of_week: CronField,
}

impl TaskScheduleCron {
    pub fn new(cron: [CronField; 6]) -> Self {
        let [seconds, minute, hour, day_of_month, month, day_of_week] = cron;
        Self {
            seconds,
            minute,
            hour,
            day_of_month,
            month,
            day_of_week
        }
    }
}

impl FromStr for TaskScheduleCron {
    type Err = CronError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tokens = tokenize_fields(s)
            .map_err(|(error_type, position, field_pos)|
                CronError {
                    field_pos,
                    position,
                    error_type: CronErrorTypes::Lexer(error_type),
                }
            )?;

        let mut cron: [AstTreeNode; 6] = Default::default();
        for (idx, toks) in tokens.into_iter().enumerate() {
            let mut parser_instance = CronParser::new(&toks);
            cron[idx] = parser_instance.parse_field()
                .map_err(|error_type|
                    CronError {
                        field_pos: idx,
                        position: parser_instance.pos,
                        error_type: CronErrorTypes::Parser(error_type),
                    }
                )?;
        }



        todo!()
    }
}

impl TaskSchedule for TaskScheduleCron {
    fn schedule(&self, _time: SystemTime) -> Result<SystemTime, Box<dyn Error + Send + Sync>> {
        todo!()
    }
}
