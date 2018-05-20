use syntax::{Token, TokenKind, Parser};
use expr::{Expression, ExpressionValue};
use asm::cpudef::{Rule, RuleParameterType, RulePatternMatcher};
use num::BigInt;
use std::collections::HashMap;


#[derive(Debug)]
pub struct CpuDef
{
	pub align: usize,
	pub rules: Vec<Rule>,
	pub pattern_matcher: RulePatternMatcher,
	pub custom_token_defs: Vec<CustomTokenDef>
}


struct CpuDefParser<'t>
{
	parser: &'t mut Parser,
	
	align: Option<usize>,
	rules: Vec<Rule>,
	custom_token_defs: Vec<CustomTokenDef>
}


#[derive(Debug)]
pub struct CustomTokenDef
{
	pub name: String,
	pub excerpt_to_value_map: HashMap<String, ExpressionValue>
}


impl CpuDef
{
	pub fn parse(parser: &mut Parser) -> Result<CpuDef, ()>
	{
		let mut cpudef_parser = CpuDefParser
		{
			parser: parser,
			align: None,
			rules: Vec::new(),
			custom_token_defs: Vec::new()
		};
		
		cpudef_parser.parse_directives()?;	
		
		if cpudef_parser.align.is_none()
			{ cpudef_parser.align = Some(8); }
		
		cpudef_parser.parse_rules()?;
		
		let pattern_matcher = RulePatternMatcher::new(&cpudef_parser.rules, &cpudef_parser.custom_token_defs);
		
		let cpudef = CpuDef
		{
			align: cpudef_parser.align.unwrap(),
			rules: cpudef_parser.rules,
			pattern_matcher: pattern_matcher,
			custom_token_defs: cpudef_parser.custom_token_defs
		};
		
		Ok(cpudef)
	}
}


impl<'t> CpuDefParser<'t>
{
	fn parse_directives(&mut self) -> Result<(), ()>
	{
		while self.parser.maybe_expect(TokenKind::Hash).is_some()
		{
			let tk_name = self.parser.expect_msg(TokenKind::Identifier, "expected directive name")?;
			match tk_name.excerpt.as_ref().unwrap().as_ref()
			{
				"align" => self.parse_directive_align(&tk_name)?,
				"tokendef" => self.parse_directive_tokendef(&tk_name)?,
				
				_ => return Err(self.parser.report.error_span("unknown directive", &tk_name.span))
			}
			
			self.parser.expect_linebreak()?;
		}
	
		Ok(())
	}
	
	
	fn parse_directive_align(&mut self, tk_name: &Token) -> Result<(), ()>
	{
		let (tk_align, align) = self.parser.expect_usize()?;
		
		if self.align.is_some()
			{ return Err(self.parser.report.error_span("duplicate align directive", &tk_name.span)); }
			
		if align == 0
			{ return Err(self.parser.report.error_span("invalid alignment", &tk_align.span)); }
		
		self.align = Some(align);
		
		Ok(())
	}
	
	
	fn parse_directive_tokendef(&mut self, _tk_name: &Token) -> Result<(), ()>
	{
		let tk_defname = self.parser.expect(TokenKind::Identifier)?;
		
		let defname = tk_defname.excerpt.unwrap().clone();
		
		if self.custom_token_defs.iter().find(|def| def.name == defname).is_some()
			{ return Err(self.parser.report.error_span("duplicate custom token def name", &tk_defname.span)); }
		
		let mut tokendef = CustomTokenDef
		{
			name: defname,
			excerpt_to_value_map: HashMap::new()
		};
		
		self.parser.expect(TokenKind::BraceOpen)?;
		
		while !self.parser.is_over() && !self.parser.next_is(0, TokenKind::BraceClose)
		{
			let tk_token = self.parser.expect(TokenKind::Identifier)?;
			let token_excerpt = tk_token.excerpt.unwrap().clone();
			
			if tokendef.excerpt_to_value_map.contains_key(&token_excerpt)
				{ return Err(self.parser.report.error_span("duplicate token in group", &tk_token.span)); }
			
			self.parser.expect(TokenKind::Equal)?;
			let value = ExpressionValue::Integer(BigInt::from(self.parser.expect_usize()?.1));
			
			tokendef.excerpt_to_value_map.insert(token_excerpt, value);
			
			if self.parser.maybe_expect_linebreak().is_some()
				{ continue; }
				
			if self.parser.next_is(0, TokenKind::BraceClose)
				{ continue; }
				
			self.parser.expect(TokenKind::Comma)?;
		}
		
		self.parser.expect(TokenKind::BraceClose)?;
		
		self.custom_token_defs.push(tokendef);
		
		Ok(())
	}
	

	fn parse_rules(&mut self) -> Result<(), ()>
	{
		while !self.parser.is_over() && !self.parser.next_is(0, TokenKind::BraceClose)
		{
			self.parse_rule()?;
			self.parser.expect_linebreak()?;
		}
		
		Ok(())
	}
	

	fn parse_rule(&mut self) -> Result<(), ()>
	{
		let mut rule = Rule::new();
		
		self.parse_rule_pattern(&mut rule)?;
		
		if rule.pattern_parts.len() == 0
		{
			let span = self.parser.next().span.before();
			return Err(self.parser.report.error_span("empty rule pattern", &span));
		}
		
		self.parser.expect(TokenKind::Arrow)?;
		self.parse_rule_production(&mut rule)?;
		
		self.rules.push(rule);
		
		Ok(())
	}
	

	fn parse_rule_pattern(&mut self, rule: &mut Rule) -> Result<(), ()>
	{
		let mut prev_was_parameter = false;
		
		
		while !self.parser.next_is(0, TokenKind::Arrow) && !self.parser.next_is(0, TokenKind::ColonColon)
		{
			let tk = self.parser.advance();
			
			// Force read an identifier at the start of the pattern.
			if rule.pattern_parts.len() == 0
			{
				if tk.kind != TokenKind::Identifier
					{ return Err(self.parser.report.error_span("expected identifier as first pattern token", &tk.span)); }
					
				rule.pattern_add_exact(&tk);
			}
			
			// Read a parameter.
			else if tk.kind == TokenKind::BraceOpen
			{
				// Check for consecutive parameters without a separating token.
				if prev_was_parameter
					{ return Err(self.parser.report.error_span("expected a separating token between parameters", &tk.span.before())); }
			
				self.parse_rule_parameter(rule)?;
				prev_was_parameter = true;
			}
			
			// Read an exact pattern part.
			else if tk.kind.is_allowed_pattern_token()
			{
				// Check for a stricter set of tokens if a parameter came just before.
				if prev_was_parameter && !tk.kind.is_allowed_after_pattern_parameter()
					{ return Err(self.parser.report.error_span("ambiguous pattern token after parameter", &tk.span)); }
				
				rule.pattern_add_exact(&tk);
				prev_was_parameter = false;
			}
			
			// Check for end of file.
			else if tk.kind == TokenKind::End
				{ return Ok(()) }
			
			// Else, it's illegal to appear in a pattern.
			else
				{ return Err(self.parser.report.error_span("invalid pattern token", &tk.span)); }
		}
	
		Ok(())
	}
	

	fn parse_rule_parameter(&mut self, rule: &mut Rule) -> Result<(), ()>
	{
		let tk_name = self.parser.expect(TokenKind::Identifier)?;
		
		let name = tk_name.excerpt.unwrap().clone();
		
		if is_reserved_var(&name)
			{ return Err(self.parser.report.error_span("reserved variable name", &tk_name.span)); }
		
		if rule.param_exists(&name)
			{ return Err(self.parser.report.error_span("duplicate parameter name", &tk_name.span)); }
			
		let typ =
			if self.parser.maybe_expect(TokenKind::Colon).is_some()
			{
				let tk_type = self.parser.expect(TokenKind::Identifier)?;
				let typename = tk_type.excerpt.unwrap().clone();
				
				let mut tokendef_index = None;
				for i in 0..self.custom_token_defs.len()
				{
					if typename == self.custom_token_defs[i].name
						{ tokendef_index = Some(i); }
				}
				
				if tokendef_index.is_none()
					{ return Err(self.parser.report.error_span("unknown parameter type", &tk_type.span)); }
						
				RuleParameterType::CustomTokenDef(tokendef_index.unwrap())
			}
			else
				{ RuleParameterType::Expression };
			
		rule.pattern_add_param(name, typ);
		
		self.parser.expect(TokenKind::BraceClose)?;
		
		Ok(())
	}
	

	fn parse_rule_production(&mut self, rule: &mut Rule) -> Result<(), ()>
	{
		let expr = Expression::parse(&mut self.parser)?;
		
		let width = match expr.width()
		{
			Some(w) => w,
			None => return Err(self.parser.report.error_span("width of expression not known; use bit slices", &expr.span()))
		};
		
		if width % self.align.unwrap() != 0
			{ return Err(self.parser.report.error_span(format!("binary representation (width = {}) does not align with a word boundary", width), &expr.span())); }
		
		rule.production = expr;
		
		Ok(())
	}
}


fn is_reserved_var(name: &str) -> bool
{
	name == "pc"
}