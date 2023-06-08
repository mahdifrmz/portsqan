use server::Input;

#[derive(Default)]
pub struct Parser {
    state: ReplState,
    fields: Vec<String>,
    pointer: usize,
    buffer: Option<Token>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Token {
    Int(usize),
    String(String),
    Tcp,
    Udp,
    True,
    False,
    EOF,
}

#[derive(Debug)]
pub enum Error {
    Empty,
    UnknownCommand,
    UnexpectedEnd,
    InvalidParam(usize),
    HostNotSpecified,
    InvalidPort(usize),
}

#[derive(Default)]
pub struct ReplState {
    pub host: Option<String>,
}

impl Parser {
    fn read(&mut self) -> Token {
        let field = if let Some(s) = self.fields.first().cloned() {
            self.fields.remove(0);
            s
        } else {
            return Token::EOF;
        };
        self.pointer += 1;
        match field.parse::<usize>() {
            Ok(val) => Token::Int(val),
            Err(_) => match field.to_lowercase().as_str() {
                "true" => Token::True,
                "false" => Token::False,
                "tcp" => Token::Tcp,
                "udp" => Token::Udp,
                _ => Token::String(field),
            },
        }
    }
    fn peek(&mut self) -> Token {
        if let Some(buf) = self.buffer.clone() {
            buf
        } else {
            let token = self.read();
            self.buffer = Some(token.clone());
            token
        }
    }
    fn next(&mut self) -> Token {
        if let Some(buf) = self.buffer.clone() {
            self.buffer = None;
            buf
        } else {
            self.read()
        }
    }
    fn parse_number(&mut self) -> Result<usize, Error> {
        let token = self.next();
        match token {
            Token::Int(value) => Ok(value),
            _ => Err(Error::InvalidParam(self.pointer)),
        }
    }
    fn parse_string(&mut self) -> Result<String, Error> {
        let token = self.next();
        match token {
            Token::String(value) => Ok(value),
            _ => Err(Error::InvalidParam(self.pointer)),
        }
    }
    fn parse_config(&mut self) -> Result<Input, Error> {
        let key = self.next();
        if let Token::String(key) = key {
            match key.as_str() {
                "threads" | "thread" | "t" => Ok(Input::Threads(self.parse_number()?)),
                _ => Err(Error::InvalidParam(self.pointer)),
            }
        } else {
            Err(Error::InvalidParam(self.pointer))
        }
    }
    fn parse_scan(&mut self) -> Result<Input, Error> {
        let host = if let Token::String(name) = self.peek() {
            let name = name;
            self.next();
            name
        } else {
            self.state.host.clone().ok_or(Error::HostNotSpecified)?
        };
        let is_tcp = if self.peek() == Token::Tcp {
            self.next();
            true
        } else if self.peek() == Token::Udp {
            self.next();
            false
        } else {
            true
        };
        let from = self.parse_number()?;
        let to = if let Token::Int(num) = self.peek() {
            num
        } else {
            from
        };
        if from > 0xffff {
            return Err(Error::InvalidPort(from));
        }
        if to > 0xffff {
            return Err(Error::InvalidPort(to));
        }
        Ok(if is_tcp {
            Input::TcpRange(host, from as u16, to as u16)
        } else {
            Input::UdpRange(host, from as u16, to as u16)
        })
    }
    pub fn parse(&mut self, state: ReplState, text: String) -> (Result<Input, Error>, ReplState) {
        self.state = state;
        let mut fields = text.split(' ').map(|s| s.to_owned()).collect::<Vec<_>>();
        let rsl = if let Some(name) = fields.first().cloned() {
            fields.remove(0);
            self.fields = fields;
            match name.as_str() {
                "quit" | "q" | "exit" => Ok(Input::End),
                "cancel" | "c" => Ok(Input::Cancel),
                "resume" | "r" => Ok(Input::Cont),
                "config" | "conf" | "cfg" => self.parse_config(),
                "scan" | "s" => self.parse_scan(),
                _ => Err(Error::UnknownCommand),
            }
        } else {
            Err(Error::Empty)
        };
        self.pointer = 0;
        self.buffer = None;
        (rsl, std::mem::take(&mut self.state))
    }
}
