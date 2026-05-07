use std::path::Path;

use crate::frontend::ast::{
    AliasDecl, Declaration, GenericDefinitionDecl, GenericUsageDecl, ImportDecl, PackageDecl,
    QualifiedName, SourceSpan, SysmlModule,
};
use crate::frontend::diagnostics::Diagnostic;
use crate::frontend::lexer::{Token, TokenKind, lex};
use crate::frontend::resolver::{
    ResolverContext, resolve_kerml_module_with_context, resolve_kerml_module_with_resolver_context,
};
use crate::frontend::transpile::{MappingBundle, transpile_module_with_source};
use crate::ir::KirDocument;
use crate::logging::{compile_timer_start, log_compile_timed_event};
use crate::paths::default_stdlib_path;

#[derive(Debug)]
pub enum KermlError {
    Io(std::io::Error),
    Diagnostic(Diagnostic),
    Kir(crate::ir::KirError),
}

impl std::fmt::Display for KermlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read kerml file: {err}"),
            Self::Diagnostic(err) => write!(f, "{err}"),
            Self::Kir(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for KermlError {}

impl From<std::io::Error> for KermlError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<Diagnostic> for KermlError {
    fn from(value: Diagnostic) -> Self {
        Self::Diagnostic(value)
    }
}

impl From<crate::ir::KirError> for KermlError {
    fn from(value: crate::ir::KirError) -> Self {
        Self::Kir(value)
    }
}

pub fn load_kerml_document(path: &Path) -> Result<KirDocument, KermlError> {
    let stdlib = KirDocument::from_path(&default_stdlib_path())?;
    load_kerml_document_with_stdlib(path, &stdlib)
}

pub fn load_kerml_document_with_stdlib(
    path: &Path,
    stdlib: &KirDocument,
) -> Result<KirDocument, KermlError> {
    let input = std::fs::read_to_string(path)?;
    compile_kerml_text(&input, &path.display().to_string(), stdlib).map_err(Into::into)
}

pub fn compile_kerml_text(
    input: &str,
    source_name: &str,
    stdlib: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    let parse_start = compile_timer_start();
    let module = parse_kerml(input)?;
    log_compile_timed_event(
        "kerml.compile.parse",
        parse_start,
        "ok",
        format!("source={} bytes={}", source_name, input.len()),
    );
    compile_kerml_module(&module, source_name, stdlib)
}

pub fn compile_kerml_text_with_context(
    input: &str,
    source_name: &str,
    context_modules: &[SysmlModule],
    stdlib: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    let parse_start = compile_timer_start();
    let module = parse_kerml(input)?;
    log_compile_timed_event(
        "kerml.compile.parse",
        parse_start,
        "ok",
        format!("source={} bytes={}", source_name, input.len()),
    );
    compile_kerml_module_with_context(&module, source_name, context_modules, stdlib)
}

pub fn compile_kerml_module(
    module: &SysmlModule,
    source_name: &str,
    stdlib: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    let mapping_start = compile_timer_start();
    let mappings = MappingBundle::load()?;
    log_compile_timed_event(
        "kerml.compile.mapping_load",
        mapping_start,
        "ok",
        format!("source={}", source_name),
    );

    let resolve_start = compile_timer_start();
    let resolved =
        resolve_kerml_module_with_context(module, std::slice::from_ref(module), stdlib, mappings)?;
    log_compile_timed_event(
        "kerml.compile.resolve",
        resolve_start,
        "ok",
        format!("source={} context_modules=1", source_name),
    );

    let transpile_start = compile_timer_start();
    let document = transpile_module_with_source(&resolved, source_name, "kerml", mappings)?;
    log_compile_timed_event(
        "kerml.compile.transpile",
        transpile_start,
        "ok",
        format!(
            "source={} elements={}",
            source_name,
            document.elements.len()
        ),
    );
    Ok(document)
}

pub fn compile_kerml_module_with_context(
    module: &SysmlModule,
    source_name: &str,
    context_modules: &[SysmlModule],
    stdlib: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    let mapping_start = compile_timer_start();
    let mappings = MappingBundle::load()?;
    log_compile_timed_event(
        "kerml.compile.mapping_load",
        mapping_start,
        "ok",
        format!("source={}", source_name),
    );

    let resolve_start = compile_timer_start();
    let resolved = resolve_kerml_module_with_context(module, context_modules, stdlib, mappings)?;
    log_compile_timed_event(
        "kerml.compile.resolve",
        resolve_start,
        "ok",
        format!(
            "source={} context_modules={}",
            source_name,
            context_modules.len()
        ),
    );

    let transpile_start = compile_timer_start();
    let document = transpile_module_with_source(&resolved, source_name, "kerml", mappings)?;
    log_compile_timed_event(
        "kerml.compile.transpile",
        transpile_start,
        "ok",
        format!(
            "source={} elements={}",
            source_name,
            document.elements.len()
        ),
    );
    Ok(document)
}

pub fn compile_kerml_module_with_resolver_context(
    module: &SysmlModule,
    source_name: &str,
    resolver_context: &ResolverContext,
    mappings: &MappingBundle,
) -> Result<KirDocument, Diagnostic> {
    let resolve_start = compile_timer_start();
    let resolved = resolve_kerml_module_with_resolver_context(module, resolver_context, mappings)?;
    log_compile_timed_event(
        "kerml.compile.resolve",
        resolve_start,
        "ok",
        format!(
            "source={} context_modules={}",
            source_name,
            resolver_context.module_count()
        ),
    );

    let transpile_start = compile_timer_start();
    let document = transpile_module_with_source(&resolved, source_name, "kerml", mappings)?;
    log_compile_timed_event(
        "kerml.compile.transpile",
        transpile_start,
        "ok",
        format!(
            "source={} elements={}",
            source_name,
            document.elements.len()
        ),
    );
    Ok(document)
}

pub fn parse_kerml(input: &str) -> Result<SysmlModule, Diagnostic> {
    let tokens = lex(input)?;
    Parser::new(tokens).parse()
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    pending_docs: Vec<String>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            index: 0,
            pending_docs: Vec::new(),
        }
    }

    fn parse(mut self) -> Result<SysmlModule, Diagnostic> {
        let mut module = SysmlModule::default();

        while !self.at_end() {
            self.collect_docs();
            let Some(declaration) = self.parse_declaration()? else {
                break;
            };
            match declaration {
                Declaration::Package(package) => {
                    if module.package.is_none() {
                        module.package = Some(package.clone());
                    }
                    module.members.push(Declaration::Package(package));
                }
                Declaration::Import(import) => {
                    module.imports.push(import.clone());
                    module.members.push(Declaration::Import(import));
                }
                Declaration::GenericDefinition(definition) => {
                    module
                        .members
                        .push(Declaration::GenericDefinition(definition));
                }
                Declaration::GenericUsage(usage) => {
                    module.members.push(Declaration::GenericUsage(usage));
                }
                Declaration::Alias(alias) => module.members.push(Declaration::Alias(alias)),
                Declaration::PartDefinition(_) | Declaration::PartUsage(_) => {
                    return Err(self.error_here("unexpected SysML-specific KerML declaration"));
                }
            }
        }

        Ok(module)
    }

    fn parse_declaration(&mut self) -> Result<Option<Declaration>, Diagnostic> {
        self.collect_docs();
        let docs = std::mem::take(&mut self.pending_docs);
        let metadata_prefixes = self.parse_metadata_prefixes();
        let modifiers = self.parse_modifiers();
        let metadata_prefixes_after_modifiers = self.parse_metadata_prefixes();
        self.skip_multiplicity();
        match self.peek_kind().clone() {
            TokenKind::Package => Ok(Some(Declaration::Package(self.parse_package(docs)?))),
            TokenKind::Import => Ok(Some(Declaration::Import(self.parse_import(docs)?))),
            TokenKind::Identifier(value) if value == "alias" => {
                Ok(Some(Declaration::Alias(self.parse_alias(docs)?)))
            }
            TokenKind::Identifier(value) if is_definition_keyword(&value) => Ok(Some(
                Declaration::GenericDefinition(self.parse_classifier(docs)?),
            )),
            TokenKind::Identifier(value)
                if value == "feature" && matches!(self.next_kind(), Some(TokenKind::Def)) =>
            {
                Ok(Some(Declaration::GenericDefinition(
                    self.parse_feature_definition(docs)?,
                )))
            }
            TokenKind::Identifier(value) if value == "feature" => Ok(Some(
                Declaration::GenericUsage(self.parse_feature_with_modifiers(docs, modifiers)?),
            )),
            TokenKind::Identifier(value)
                if value == "comment" || value == "locale" || value == "doc" =>
            {
                Ok(Some(Declaration::GenericUsage(
                    self.parse_opaque_declaration(docs, modifiers)?,
                )))
            }
            TokenKind::Identifier(value)
                if !modifiers.is_empty() || self.starts_unprefixed_feature(&value) =>
            {
                Ok(Some(Declaration::GenericUsage(
                    self.parse_unprefixed_feature(docs, modifiers)?,
                )))
            }
            TokenKind::Eof => Ok(None),
            TokenKind::Identifier(_) | TokenKind::Specializes | TokenKind::Redefines => Ok(Some(
                Declaration::GenericUsage(self.parse_opaque_declaration(docs, modifiers)?),
            )),
            TokenKind::LBrace | TokenKind::Semicolon
                if !metadata_prefixes.is_empty()
                    || !metadata_prefixes_after_modifiers.is_empty() =>
            {
                Ok(Some(Declaration::GenericUsage(
                    self.parse_opaque_declaration(docs, modifiers)?,
                )))
            }
            TokenKind::RBrace => Ok(None),
            _ => Err(self.error_here(
                "expected a KerML declaration such as `package`, `import`, `classifier`, or `feature`",
            )),
        }
    }

    fn parse_package(&mut self, docs: Vec<String>) -> Result<PackageDecl, Diagnostic> {
        let start = self.expect(TokenKind::Package, "expected `package`")?;
        let name = self.parse_qualified_name()?;
        let _specializes = self.parse_optional_specializations()?;
        if matches!(self.peek_kind(), TokenKind::Semicolon) {
            let end = self.expect(TokenKind::Semicolon, "expected `;` after package")?;
            return Ok(PackageDecl {
                name,
                members: Vec::new(),
                imports: Vec::new(),
                definitions: Vec::new(),
                docs,
                modifiers: Vec::new(),
                span: merge_span(&start.span, &end.span),
            });
        }
        self.expect(TokenKind::LBrace, "expected `{` after package name")?;

        let mut members = Vec::new();
        let mut imports = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            let Some(declaration) = self.parse_declaration()? else {
                break;
            };
            if let Declaration::Import(import) = &declaration {
                imports.push(import.clone());
            }
            members.push(declaration);
        }
        let end = self.expect(TokenKind::RBrace, "expected `}` to close package")?;

        Ok(PackageDecl {
            name,
            members,
            imports,
            definitions: Vec::new(),
            docs,
            modifiers: Vec::new(),
            span: merge_span(&start.span, &end.span),
        })
    }

    fn parse_import(&mut self, docs: Vec<String>) -> Result<ImportDecl, Diagnostic> {
        let start = self.expect(TokenKind::Import, "expected `import`")?;
        let path = self.parse_import_path()?;
        let end = if matches!(self.peek_kind(), TokenKind::Semicolon) {
            self.expect(TokenKind::Semicolon, "expected `;` after import")?
        } else {
            self.consume_declaration_tail()
        };
        Ok(ImportDecl {
            path,
            docs,
            modifiers: Vec::new(),
            span: merge_span(&start.span, &end.span),
        })
    }

    fn parse_alias(&mut self, docs: Vec<String>) -> Result<AliasDecl, Diagnostic> {
        let start = self.expect_identifier_named("alias", "expected `alias`")?;
        let name = self.expect_identifier("expected alias name")?;
        if matches!(self.peek_kind(), TokenKind::Identifier(value) if value == "for") {
            self.advance();
        } else {
            self.expect(TokenKind::Equals, "expected `=` after alias name")?;
        }
        let target = self.parse_qualified_name()?;
        let end = self.expect(TokenKind::Semicolon, "expected `;` after alias")?;
        Ok(AliasDecl {
            name,
            target,
            docs,
            modifiers: Vec::new(),
            span: merge_span(&start.span, &end.span),
        })
    }

    fn parse_classifier(&mut self, docs: Vec<String>) -> Result<GenericDefinitionDecl, Diagnostic> {
        let start = self.expect_identifier_token("expected classifier keyword")?;
        let keyword = token_identifier(&start).to_string();
        if matches!(self.peek_kind(), TokenKind::Identifier(value) if is_definition_keyword(value))
        {
            self.advance();
        }
        self.skip_angle_metadata();
        self.parse_modifiers();
        let name = self.expect_identifier("expected classifier name")?;
        self.skip_multiplicity();
        let specializes = self.parse_classifier_relations()?;

        let mut members = Vec::new();
        let end = match self.peek_kind() {
            TokenKind::Semicolon => self.expect(TokenKind::Semicolon, "expected `;`")?,
            TokenKind::RBrace => self.current().clone(),
            TokenKind::LBrace => {
                self.advance();
                while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                    let Some(declaration) = self.parse_declaration()? else {
                        break;
                    };
                    members.push(declaration);
                }
                self.expect(TokenKind::RBrace, "expected `}` to close classifier")?
            }
            _ => return Err(self.error_here("expected `;` or `{` after classifier declaration")),
        };

        Ok(GenericDefinitionDecl {
            keyword,
            name,
            specializes,
            members,
            docs,
            modifiers: Vec::new(),
            span: merge_span(&start.span, &end.span),
        })
    }

    fn parse_feature_definition(
        &mut self,
        docs: Vec<String>,
    ) -> Result<GenericDefinitionDecl, Diagnostic> {
        let start = self.expect_identifier_named("feature", "expected `feature`")?;
        self.expect(TokenKind::Def, "expected `def` after `feature`")?;
        let name = self.expect_identifier("expected feature definition name")?;
        let specializes = self.parse_optional_specializations()?;

        let mut members = Vec::new();
        let end = match self.peek_kind() {
            TokenKind::Semicolon => self.expect(TokenKind::Semicolon, "expected `;`")?,
            TokenKind::RBrace => self.current().clone(),
            TokenKind::LBrace => {
                self.advance();
                while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                    let Some(declaration) = self.parse_declaration()? else {
                        break;
                    };
                    members.push(declaration);
                }
                self.expect(
                    TokenKind::RBrace,
                    "expected `}` to close feature definition",
                )?
            }
            _ => return Err(self.error_here("expected `;` or `{` after feature definition")),
        };

        Ok(GenericDefinitionDecl {
            keyword: "feature".to_string(),
            name,
            specializes,
            members,
            docs,
            modifiers: Vec::new(),
            span: merge_span(&start.span, &end.span),
        })
    }

    fn parse_feature_with_modifiers(
        &mut self,
        docs: Vec<String>,
        modifiers: Vec<String>,
    ) -> Result<GenericUsageDecl, Diagnostic> {
        let start = self.expect_identifier_named("feature", "expected `feature`")?;
        let mut name = if matches!(
            self.peek_kind(),
            TokenKind::Colon | TokenKind::Specializes | TokenKind::Redefines
        ) {
            format!("feature_{}_{}", start.span.start_line, start.span.start_col)
        } else {
            self.expect_identifier("expected feature name")?
        };
        if is_relation_keyword_name(&name) {
            if let TokenKind::Identifier(value) = self.peek_kind().clone() {
                name = value;
                self.advance();
            }
        }
        self.skip_multiplicity();
        let mut additional_types = Vec::new();
        let ty = if matches!(self.peek_kind(), TokenKind::Colon) {
            self.advance();
            let ty = self.parse_qualified_name()?;
            self.skip_multiplicity();
            while matches!(self.peek_kind(), TokenKind::Comma) {
                self.advance();
                additional_types.push(self.parse_qualified_name()?);
                self.skip_multiplicity();
            }
            Some(ty)
        } else {
            None
        };
        let relations = self.parse_optional_feature_relations()?;
        if matches!(self.peek_kind(), TokenKind::Equals) {
            self.skip_expression_tail();
        } else {
            self.skip_feature_tail();
        }

        let mut body_members = Vec::new();
        let end = match self.peek_kind() {
            TokenKind::Semicolon => self.expect(TokenKind::Semicolon, "expected `;`")?,
            TokenKind::RBrace => self.current().clone(),
            TokenKind::LBrace => {
                self.advance();
                while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                    let Some(declaration) = self.parse_declaration()? else {
                        break;
                    };
                    body_members.push(declaration);
                }
                self.expect(TokenKind::RBrace, "expected `}` to close feature")?
            }
            _ => return Err(self.error_here("expected `;` or `{` after feature declaration")),
        };

        Ok(GenericUsageDecl {
            keyword: "feature".to_string(),
            name,
            is_implicit_name: false,
            ty,
            reference_target: None,
            multiplicity: None,
            expression: None,
            additional_types,
            specializes: relations.specializes,
            subsets: relations.subsets,
            redefines: relations.redefines,
            body_members,
            docs,
            modifiers,
            span: merge_span(&start.span, &end.span),
        })
    }

    fn parse_unprefixed_feature(
        &mut self,
        docs: Vec<String>,
        modifiers: Vec<String>,
    ) -> Result<GenericUsageDecl, Diagnostic> {
        let start = self.current().clone();
        self.skip_multiplicity();
        self.skip_angle_metadata();
        let mut name = if matches!(
            self.peek_kind(),
            TokenKind::Colon | TokenKind::Specializes | TokenKind::Redefines
        ) {
            format!("feature_{}_{}", start.span.start_line, start.span.start_col)
        } else {
            self.expect_identifier("expected feature name")?
        };
        self.skip_multiplicity();
        if matches!(self.peek_kind(), TokenKind::Identifier(value) if value == "feature") {
            self.advance();
            name = if matches!(
                self.peek_kind(),
                TokenKind::Colon | TokenKind::Specializes | TokenKind::Redefines
            ) {
                format!("feature_{}_{}", start.span.start_line, start.span.start_col)
            } else {
                self.expect_identifier("expected feature name")?
            };
        }
        if is_relation_keyword_name(&name) {
            if let TokenKind::Identifier(value) = self.peek_kind().clone() {
                name = value;
                self.advance();
            }
        }
        let mut additional_types = Vec::new();
        let ty = if matches!(self.peek_kind(), TokenKind::Colon) {
            self.advance();
            let ty = self.parse_qualified_name()?;
            self.skip_multiplicity();
            while matches!(self.peek_kind(), TokenKind::Comma) {
                self.advance();
                additional_types.push(self.parse_qualified_name()?);
                self.skip_multiplicity();
            }
            Some(ty)
        } else {
            None
        };
        let relations = self.parse_optional_feature_relations()?;
        if matches!(self.peek_kind(), TokenKind::Equals) {
            self.skip_expression_tail();
        } else {
            self.skip_feature_tail();
        }

        let mut body_members = Vec::new();
        let end = match self.peek_kind() {
            TokenKind::Semicolon => self.expect(TokenKind::Semicolon, "expected `;`")?,
            TokenKind::RBrace => self.current().clone(),
            TokenKind::LBrace => {
                self.advance();
                while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                    let Some(declaration) = self.parse_declaration()? else {
                        break;
                    };
                    body_members.push(declaration);
                }
                self.expect(TokenKind::RBrace, "expected `}` to close feature")?
            }
            _ => return Err(self.error_here("expected `;` or `{` after feature declaration")),
        };

        Ok(GenericUsageDecl {
            keyword: "feature".to_string(),
            name,
            is_implicit_name: false,
            ty,
            reference_target: None,
            multiplicity: None,
            expression: None,
            additional_types,
            specializes: relations.specializes,
            subsets: relations.subsets,
            redefines: relations.redefines,
            body_members,
            docs,
            modifiers,
            span: merge_span(&start.span, &end.span),
        })
    }

    fn parse_opaque_declaration(
        &mut self,
        docs: Vec<String>,
        modifiers: Vec<String>,
    ) -> Result<GenericUsageDecl, Diagnostic> {
        let start = self.current().clone();
        let keyword = match self.peek_kind().clone() {
            TokenKind::Identifier(value) => {
                self.advance();
                value
            }
            TokenKind::Specializes => {
                self.advance();
                "specialization".to_string()
            }
            TokenKind::Redefines => {
                self.advance();
                "redefinition".to_string()
            }
            _ => "declaration".to_string(),
        };
        self.skip_angle_metadata();
        let name = match self.peek_kind().clone() {
            TokenKind::Identifier(value) => {
                self.advance();
                value
            }
            _ => keyword.clone(),
        };

        let end = self.consume_declaration_tail();
        Ok(GenericUsageDecl {
            keyword,
            name,
            is_implicit_name: false,
            ty: None,
            reference_target: None,
            multiplicity: None,
            expression: None,
            additional_types: Vec::new(),
            specializes: Vec::new(),
            subsets: Vec::new(),
            redefines: Vec::new(),
            body_members: Vec::new(),
            docs,
            modifiers,
            span: merge_span(&start.span, &end.span),
        })
    }

    fn parse_optional_specializations(&mut self) -> Result<Vec<QualifiedName>, Diagnostic> {
        let mut specializes = Vec::new();
        if matches!(self.peek_kind(), TokenKind::Specializes)
            || matches!(self.peek_kind(), TokenKind::Identifier(value) if value == "specializes")
        {
            self.advance();
            specializes.push(self.parse_qualified_name()?);
            while matches!(self.peek_kind(), TokenKind::Comma) {
                self.advance();
                specializes.push(self.parse_qualified_name()?);
            }
        }
        Ok(specializes)
    }

    fn parse_classifier_relations(&mut self) -> Result<Vec<QualifiedName>, Diagnostic> {
        let mut specializes = Vec::new();
        loop {
            match self.peek_kind().clone() {
                TokenKind::Specializes => {
                    self.advance();
                    specializes.extend(self.parse_relation_targets()?);
                }
                TokenKind::Identifier(value) if value == "specializes" => {
                    self.advance();
                    specializes.extend(self.parse_relation_targets()?);
                }
                TokenKind::Identifier(value)
                    if matches!(
                        value.as_str(),
                        "unions"
                            | "intersects"
                            | "differences"
                            | "disjoint"
                            | "conjugates"
                            | "inverse"
                            | "chains"
                            | "ordered"
                    ) =>
                {
                    self.advance();
                    if value == "disjoint"
                        && matches!(self.peek_kind(), TokenKind::Identifier(next) if next == "from")
                    {
                        self.advance();
                    }
                    if value == "inverse"
                        && matches!(self.peek_kind(), TokenKind::Identifier(next) if next == "of")
                    {
                        self.advance();
                    }
                    let _ = self.parse_relation_targets()?;
                }
                _ => break,
            }
        }
        Ok(specializes)
    }

    fn parse_optional_feature_relations(&mut self) -> Result<FeatureRelations, Diagnostic> {
        let mut relations = FeatureRelations::default();
        loop {
            match self.peek_kind().clone() {
                TokenKind::Specializes => {
                    self.advance();
                    relations.specializes.extend(self.parse_relation_targets()?);
                }
                TokenKind::Identifier(value) if value == "specializes" => {
                    self.advance();
                    relations.specializes.extend(self.parse_relation_targets()?);
                }
                TokenKind::Redefines => {
                    self.advance();
                    relations.redefines.extend(self.parse_relation_targets()?);
                }
                TokenKind::Identifier(value) if value == "redefines" => {
                    self.advance();
                    relations.redefines.extend(self.parse_relation_targets()?);
                }
                TokenKind::Identifier(value) if value == "subsets" => {
                    self.advance();
                    relations.subsets.extend(self.parse_relation_targets()?);
                }
                TokenKind::Identifier(value) if value == "typed" => {
                    self.advance();
                    self.expect_identifier_named("by", "expected `by` after `typed`")?;
                    relations.specializes.extend(self.parse_relation_targets()?);
                }
                TokenKind::Identifier(value) if value == "references" => {
                    self.advance();
                    let _ = self.parse_relation_targets()?;
                }
                TokenKind::Identifier(value) if value == "featured" => {
                    self.advance();
                    self.expect_identifier_named("by", "expected `by` after `featured`")?;
                    let _ = self.parse_relation_targets()?;
                }
                TokenKind::Identifier(value)
                    if matches!(
                        value.as_str(),
                        "unions" | "intersects" | "differences" | "disjoint" | "conjugates"
                    ) =>
                {
                    self.advance();
                    if value == "disjoint"
                        && matches!(self.peek_kind(), TokenKind::Identifier(next) if next == "from")
                    {
                        self.advance();
                    }
                    let _ = self.parse_relation_targets()?;
                }
                _ => break,
            }
        }
        Ok(relations)
    }

    fn parse_relation_targets(&mut self) -> Result<Vec<QualifiedName>, Diagnostic> {
        let mut targets = vec![self.parse_qualified_name()?];
        while matches!(self.peek_kind(), TokenKind::Comma) {
            self.advance();
            targets.push(self.parse_qualified_name()?);
        }
        Ok(targets)
    }

    fn parse_import_path(&mut self) -> Result<QualifiedName, Diagnostic> {
        let first = self.expect_identifier_token("expected import path")?;
        let mut segments = vec![token_identifier(&first).to_string()];
        let mut end = first.span.clone();
        while matches!(self.peek_kind(), TokenKind::ScopeSep | TokenKind::Dot) {
            self.advance();
            let next = self.expect_path_segment("expected import path segment", true)?;
            segments.push(token_path_segment(&next).to_string());
            end = next.span.clone();
        }
        Ok(QualifiedName {
            segments,
            span: merge_span(&first.span, &end),
        })
    }

    fn parse_qualified_name(&mut self) -> Result<QualifiedName, Diagnostic> {
        let first = self.expect_name_token("expected name")?;
        let mut segments = vec![token_name(&first).to_string()];
        let mut end = first.span.clone();
        while matches!(self.peek_kind(), TokenKind::ScopeSep | TokenKind::Dot) {
            self.advance();
            let next = self.expect_name_token("expected name segment")?;
            segments.push(token_name(&next).to_string());
            end = next.span.clone();
        }
        Ok(QualifiedName {
            segments,
            span: merge_span(&first.span, &end),
        })
    }

    fn parse_modifiers(&mut self) -> Vec<String> {
        let mut modifiers = Vec::new();
        while let TokenKind::Identifier(value) = self.peek_kind().clone() {
            if is_modifier(&value) {
                modifiers.push(value);
                self.advance();
            } else {
                break;
            }
        }
        modifiers
    }

    fn parse_metadata_prefixes(&mut self) -> Vec<String> {
        let mut prefixes = Vec::new();
        while matches!(self.peek_kind(), TokenKind::At | TokenKind::Hash) {
            self.advance();
            prefixes.push(match self.peek_kind().clone() {
                TokenKind::Identifier(value) | TokenKind::String(value) => {
                    self.advance();
                    value
                }
                _ => "metadata".to_string(),
            });
        }
        prefixes
    }

    fn starts_unprefixed_feature(&self, value: &str) -> bool {
        !is_definition_keyword(value)
            && !matches!(value, "alias" | "feature")
            && matches!(
                self.next_kind(),
                Some(
                    TokenKind::Colon
                        | TokenKind::Specializes
                        | TokenKind::Redefines
                        | TokenKind::Equals
                        | TokenKind::Semicolon
                        | TokenKind::LBrace
                )
            )
    }

    fn skip_angle_metadata(&mut self) {
        while matches!(self.peek_kind(), TokenKind::LAngle) {
            self.skip_balanced(TokenKind::LAngle, TokenKind::RAngle);
        }
    }

    fn skip_multiplicity(&mut self) {
        if matches!(self.peek_kind(), TokenKind::LBracket) {
            self.skip_balanced(TokenKind::LBracket, TokenKind::RBracket);
        }
    }

    fn skip_expression_tail(&mut self) {
        if !matches!(self.peek_kind(), TokenKind::Equals) {
            return;
        }
        let mut brace_depth = 0usize;
        let mut paren_depth = 0usize;
        let mut bracket_depth = 0usize;
        let mut angle_depth = 0usize;
        while !matches!(self.peek_kind(), TokenKind::Eof) {
            match self.peek_kind() {
                TokenKind::Semicolon
                    if brace_depth == 0
                        && paren_depth == 0
                        && bracket_depth == 0
                        && angle_depth == 0 =>
                {
                    break;
                }
                TokenKind::RBrace
                    if brace_depth == 0
                        && paren_depth == 0
                        && bracket_depth == 0
                        && angle_depth == 0 =>
                {
                    break;
                }
                TokenKind::LBrace => brace_depth += 1,
                TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
                TokenKind::LParen => paren_depth += 1,
                TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
                TokenKind::LBracket => bracket_depth += 1,
                TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                TokenKind::LAngle => angle_depth += 1,
                TokenKind::RAngle => angle_depth = angle_depth.saturating_sub(1),
                _ => {}
            }
            self.advance();
        }
    }

    fn skip_feature_tail(&mut self) {
        let mut paren_depth = 0usize;
        let mut bracket_depth = 0usize;
        let mut angle_depth = 0usize;
        while !self.at_end() {
            match self.peek_kind() {
                TokenKind::Semicolon
                    if paren_depth == 0 && bracket_depth == 0 && angle_depth == 0 =>
                {
                    break;
                }
                TokenKind::LBrace if paren_depth == 0 && bracket_depth == 0 && angle_depth == 0 => {
                    break;
                }
                TokenKind::RBrace if paren_depth == 0 && bracket_depth == 0 && angle_depth == 0 => {
                    break;
                }
                TokenKind::LParen => paren_depth += 1,
                TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
                TokenKind::LBracket => bracket_depth += 1,
                TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                TokenKind::LAngle => angle_depth += 1,
                TokenKind::RAngle => angle_depth = angle_depth.saturating_sub(1),
                _ => {}
            }
            self.advance();
        }
    }

    fn consume_declaration_tail(&mut self) -> Token {
        let mut end = self.current().clone();
        let started_with_lbrace = matches!(self.peek_kind(), TokenKind::LBrace);
        let mut brace_depth = 0usize;
        let mut paren_depth = 0usize;
        let mut bracket_depth = 0usize;
        let mut angle_depth = 0usize;

        while !self.at_end() {
            end = self.current().clone();
            match self.peek_kind() {
                TokenKind::LBrace => brace_depth += 1,
                TokenKind::RBrace => {
                    if brace_depth == 0 {
                        break;
                    }
                    brace_depth -= 1;
                }
                TokenKind::LParen => paren_depth += 1,
                TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
                TokenKind::LBracket => bracket_depth += 1,
                TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                TokenKind::LAngle => angle_depth += 1,
                TokenKind::RAngle => angle_depth = angle_depth.saturating_sub(1),
                TokenKind::Semicolon
                    if brace_depth == 0
                        && paren_depth == 0
                        && bracket_depth == 0
                        && angle_depth == 0 =>
                {
                    self.advance();
                    return end;
                }
                _ => {}
            }
            self.advance();
            if started_with_lbrace && matches!(end.kind, TokenKind::RBrace) && brace_depth == 0 {
                return end;
            }
        }

        end
    }

    fn skip_balanced(&mut self, open: TokenKind, close: TokenKind) {
        let open_discriminant = std::mem::discriminant(&open);
        let close_discriminant = std::mem::discriminant(&close);
        if std::mem::discriminant(self.peek_kind()) != open_discriminant {
            return;
        }
        let mut depth = 0usize;
        while !self.at_end() {
            let discriminant = std::mem::discriminant(self.peek_kind());
            if discriminant == open_discriminant {
                depth += 1;
            } else if discriminant == close_discriminant {
                depth = depth.saturating_sub(1);
                self.advance();
                if depth == 0 {
                    break;
                }
                continue;
            }
            self.advance();
        }
    }

    fn collect_docs(&mut self) {
        while let TokenKind::Doc(text) = self.peek_kind().clone() {
            self.pending_docs.push(text);
            self.advance();
        }
    }

    fn expect_identifier(&mut self, message: &str) -> Result<String, Diagnostic> {
        let token = self.expect_identifier_token(message)?;
        Ok(token_identifier(&token).to_string())
    }

    fn expect_identifier_named(
        &mut self,
        expected: &str,
        message: &str,
    ) -> Result<Token, Diagnostic> {
        let token = self.expect_identifier_token(message)?;
        if token_identifier(&token) == expected {
            Ok(token)
        } else {
            Err(Diagnostic::new(message, Some(token.span)))
        }
    }

    fn expect_identifier_token(&mut self, message: &str) -> Result<Token, Diagnostic> {
        let token = self.current().clone();
        match &token.kind {
            TokenKind::Identifier(_) => {
                self.advance();
                Ok(token)
            }
            _ => Err(Diagnostic::new(message, Some(token.span))),
        }
    }

    fn expect_name_token(&mut self, message: &str) -> Result<Token, Diagnostic> {
        let token = self.current().clone();
        match &token.kind {
            TokenKind::Identifier(_) | TokenKind::String(_) | TokenKind::Dollar => {
                self.advance();
                Ok(token)
            }
            _ => Err(Diagnostic::new(message, Some(token.span))),
        }
    }

    fn expect_path_segment(
        &mut self,
        message: &str,
        allow_wildcards: bool,
    ) -> Result<Token, Diagnostic> {
        let token = self.current().clone();
        match &token.kind {
            TokenKind::Identifier(_) | TokenKind::Star | TokenKind::DoubleStar
                if allow_wildcards || matches!(token.kind, TokenKind::Identifier(_)) =>
            {
                self.advance();
                Ok(token)
            }
            _ => Err(Diagnostic::new(message, Some(token.span))),
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<Token, Diagnostic> {
        let token = self.current().clone();
        if std::mem::discriminant(&token.kind) == std::mem::discriminant(&kind) {
            self.advance();
            Ok(token)
        } else {
            Err(Diagnostic::new(message, Some(token.span)))
        }
    }

    fn error_here(&self, message: &str) -> Diagnostic {
        Diagnostic::new(message, Some(self.current().span.clone()))
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn next_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.index + 1).map(|token| &token.kind)
    }

    fn advance(&mut self) {
        if !self.at_end() {
            self.index += 1;
        }
    }

    fn at_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }
}

#[derive(Debug, Default)]
struct FeatureRelations {
    specializes: Vec<QualifiedName>,
    subsets: Vec<QualifiedName>,
    redefines: Vec<QualifiedName>,
}

fn token_identifier(token: &Token) -> &str {
    match &token.kind {
        TokenKind::Identifier(value) => value,
        _ => unreachable!(),
    }
}

fn token_name(token: &Token) -> &str {
    match &token.kind {
        TokenKind::Identifier(value) | TokenKind::String(value) => value,
        TokenKind::Dollar => "$",
        _ => unreachable!(),
    }
}

fn token_path_segment(token: &Token) -> &str {
    match &token.kind {
        TokenKind::Identifier(value) => value,
        TokenKind::Star => "*",
        TokenKind::DoubleStar => "**",
        _ => unreachable!(),
    }
}

fn is_definition_keyword(value: &str) -> bool {
    matches!(
        value,
        "classifier"
            | "class"
            | "struct"
            | "datatype"
            | "behavior"
            | "function"
            | "predicate"
            | "interaction"
            | "association"
            | "assoc"
            | "metaclass"
    )
}

fn is_modifier(value: &str) -> bool {
    matches!(
        value,
        "public"
            | "private"
            | "protected"
            | "library"
            | "abstract"
            | "all"
            | "composite"
            | "portion"
            | "const"
            | "member"
            | "readonly"
            | "derived"
            | "end"
            | "in"
            | "out"
            | "inout"
            | "ref"
            | "nonunique"
            | "ordered"
    )
}

fn is_relation_keyword_name(value: &str) -> bool {
    matches!(
        value,
        "redefines" | "subsets" | "specializes" | "typed" | "featured"
    )
}

fn merge_span(left: &SourceSpan, right: &SourceSpan) -> SourceSpan {
    SourceSpan {
        start_line: left.start_line,
        start_col: left.start_col,
        end_line: right.end_line,
        end_col: right.end_col,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    #[test]
    fn parses_minimal_classifier_and_feature() {
        let module = parse_kerml(
            "package Demo {
                classifier Vehicle {
                    feature engine : Engine;
                }
                classifier Engine;
            }",
        )
        .unwrap();

        let package = module.package.unwrap();
        assert_eq!(package.name.as_dot_string(), "Demo");
        assert_eq!(package.members.len(), 2);
    }

    #[test]
    fn parses_wildcard_imports() {
        let module = parse_kerml(
            "package Demo {
                import Domain::Vehicles::*;
                import Domain.Analysis.**;
            }",
        )
        .unwrap();

        let package = module.package.unwrap();
        let imports = package.imports;
        assert_eq!(imports[0].path.as_dot_string(), "Domain.Vehicles.*");
        assert_eq!(imports[1].path.as_dot_string(), "Domain.Analysis.**");
    }

    #[test]
    fn parses_feature_relationship_tails() {
        let module = parse_kerml(
            "package Demo {
                classifier Vehicle {
                    feature base;
                    feature engine : Engine :> poweredFeature subsets base redefines oldEngine;
                }
                classifier Engine;
            }",
        )
        .unwrap();

        let package = module.package.unwrap();
        let classifier = match &package.members[0] {
            Declaration::GenericDefinition(definition) => definition,
            _ => panic!("expected classifier"),
        };
        let feature = match &classifier.members[1] {
            Declaration::GenericUsage(usage) => usage,
            _ => panic!("expected feature"),
        };
        assert_eq!(feature.name, "engine");
        assert_eq!(feature.specializes[0].as_dot_string(), "poweredFeature");
        assert_eq!(feature.subsets[0].as_dot_string(), "base");
        assert_eq!(feature.redefines[0].as_dot_string(), "oldEngine");
    }

    #[test]
    fn compiles_minimal_classifier_and_feature_to_kir() {
        let stdlib = KirDocument {
            metadata: Default::default(),
            elements: Vec::new(),
        };
        let document = compile_kerml_text(
            "package Demo {
                classifier Vehicle {
                    feature engine : Engine;
                }
                classifier Engine;
            }",
            "inline.kerml",
            &stdlib,
        )
        .unwrap();

        assert!(document.elements.iter().any(|element| {
            element.id == "type.Demo.Vehicle" && element.kind == "KerML::Core::Type"
        }));
        assert!(document.elements.iter().any(|element| {
            element.id == "feature.Demo.Vehicle.engine"
                && element.kind == "KerML::Core::Feature"
                && element.properties.get("type")
                    == Some(&Value::String("type.Demo.Engine".to_string()))
        }));
    }
}
