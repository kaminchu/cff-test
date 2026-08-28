use std::{collections::HashSet, path::Path};

use swc_common::{DUMMY_SP, FileName, SourceMap, Span, Spanned, sync::Lrc};
use swc_ecma_ast::*;
use swc_ecma_parser::{EsSyntax, Parser, StringInput, Syntax, lexer::Lexer};
use swc_ecma_visit::{Visit, VisitWith};

use super::{diagnostic::Diagnostic, runtime20};

pub fn check_source(path: &Path, source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if source.len() > 10 * 1024 {
        diagnostics.push(make_diagnostic(
            path,
            source,
            DUMMY_SP,
            "CFF010",
            "function code is larger than 10 KiB",
            "reduce the UTF-8 source to at most 10 * 1024 bytes",
        ));
        return diagnostics;
    }
    let source_map: Lrc<SourceMap> = Default::default();
    let file = source_map.new_source_file(
        Lrc::new(FileName::Custom(path.display().to_string())),
        source.to_string(),
    );
    let lexer = Lexer::new(
        Syntax::Es(EsSyntax::default()),
        EsVersion::Es2022,
        StringInput::from(&*file),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let module = match parser.parse_module() {
        Ok(module) => module,
        Err(error) => {
            diagnostics.push(make_diagnostic(
                path,
                source,
                error.span(),
                "CFF001",
                &format!("JavaScript syntax error: {error:?}"),
                "fix the syntax error before running the function",
            ));
            return diagnostics;
        }
    };
    for error in parser.take_errors() {
        diagnostics.push(make_diagnostic(
            path,
            source,
            error.span(),
            "CFF001",
            &format!("JavaScript syntax error: {error:?}"),
            "fix the recovered syntax error before running the function",
        ));
    }
    if !diagnostics.is_empty() {
        return diagnostics;
    }

    let mut bindings = BindingCollector::default();
    module.visit_with(&mut bindings);
    let mut visitor = RuleVisitor {
        path,
        source,
        bindings: bindings.names,
        cloudfront_bindings: HashSet::new(),
        forbidden_aliases: HashSet::new(),
        diagnostics: Vec::new(),
        async_depth: 0,
    };
    for item in &module.body {
        if let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = item {
            let name = import.src.value.as_wtf8().as_str().unwrap_or("");
            if !runtime20::SUPPORTED_MODULES.contains(&name) {
                visitor.add(
                    import.span,
                    "CFF008",
                    format!("module '{name}' is not available in CloudFront Functions runtime 2.0"),
                    "import only crypto, querystring, or cloudfront",
                );
            }
            if name == "cloudfront" {
                for specifier in &import.specifiers {
                    if let ImportSpecifier::Named(named) = specifier {
                        let imported = named
                            .imported
                            .as_ref()
                            .map(|name| name.atom().to_string())
                            .unwrap_or_else(|| named.local.sym.to_string());
                        if imported != "default" && imported != "kvs" {
                            visitor.add(
                                named.span,
                                "CFF012",
                                format!(
                                    "cloudfront export '{imported}' is not supported by this version of cff-test"
                                ),
                                "use the default cloudfront import and cloudfront.kvs() only",
                            );
                        }
                    }
                    let local = match specifier {
                        ImportSpecifier::Named(specifier) => &specifier.local,
                        ImportSpecifier::Default(specifier) => &specifier.local,
                        ImportSpecifier::Namespace(specifier) => &specifier.local,
                    };
                    visitor.cloudfront_bindings.insert(local.sym.to_string());
                }
            }
        }
    }
    module.visit_with(&mut visitor);
    check_handler(path, source, &module, &mut visitor.diagnostics);
    visitor.diagnostics.sort_by(|left, right| {
        (left.byte_start, left.rule, &left.message).cmp(&(
            right.byte_start,
            right.rule,
            &right.message,
        ))
    });
    visitor.diagnostics.dedup_by(|left, right| {
        left.byte_start == right.byte_start
            && left.rule == right.rule
            && left.message == right.message
    });
    visitor.diagnostics
}

#[derive(Default)]
struct BindingCollector {
    names: HashSet<String>,
}
impl BindingCollector {
    fn pattern(&mut self, pattern: &Pat) {
        match pattern {
            Pat::Ident(binding) => {
                self.names.insert(binding.id.sym.to_string());
            }
            Pat::Array(array) => {
                for pattern in array.elems.iter().flatten() {
                    self.pattern(pattern);
                }
            }
            Pat::Object(object) => {
                for property in &object.props {
                    match property {
                        ObjectPatProp::KeyValue(property) => self.pattern(&property.value),
                        ObjectPatProp::Assign(property) => {
                            self.names.insert(property.key.sym.to_string());
                        }
                        ObjectPatProp::Rest(property) => self.pattern(&property.arg),
                    }
                }
            }
            Pat::Rest(rest) => self.pattern(&rest.arg),
            Pat::Assign(assign) => self.pattern(&assign.left),
            _ => {}
        }
    }
}
impl Visit for BindingCollector {
    fn visit_var_declarator(&mut self, node: &VarDeclarator) {
        self.pattern(&node.name);
        node.init.visit_with(self);
    }
    fn visit_fn_decl(&mut self, node: &FnDecl) {
        self.names.insert(node.ident.sym.to_string());
        for param in &node.function.params {
            self.pattern(&param.pat);
        }
        node.function.body.visit_with(self);
    }
    fn visit_fn_expr(&mut self, node: &FnExpr) {
        if let Some(ident) = &node.ident {
            self.names.insert(ident.sym.to_string());
        }
        for param in &node.function.params {
            self.pattern(&param.pat);
        }
        node.function.body.visit_with(self);
    }
    fn visit_arrow_expr(&mut self, node: &ArrowExpr) {
        for param in &node.params {
            self.pattern(param);
        }
        node.body.visit_with(self);
    }
    fn visit_catch_clause(&mut self, node: &CatchClause) {
        if let Some(param) = &node.param {
            self.pattern(param);
        }
        node.body.visit_with(self);
    }
    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.names.insert(node.ident.sym.to_string());
        node.class.visit_with(self);
    }
}

struct RuleVisitor<'a> {
    path: &'a Path,
    source: &'a str,
    bindings: HashSet<String>,
    cloudfront_bindings: HashSet<String>,
    forbidden_aliases: HashSet<String>,
    diagnostics: Vec<Diagnostic>,
    async_depth: usize,
}
impl RuleVisitor<'_> {
    fn add(
        &mut self,
        span: Span,
        rule: &'static str,
        message: impl Into<String>,
        help: impl Into<String>,
    ) {
        let message = message.into();
        let help = help.into();
        self.diagnostics.push(make_diagnostic(
            self.path,
            self.source,
            span,
            rule,
            &message,
            &help,
        ));
    }
    fn bound(&self, name: &str) -> bool {
        self.bindings.contains(name)
    }
    fn member_name<'a>(&self, member: &'a MemberExpr) -> Option<(&'a str, &'a str)> {
        let base = match &*member.obj {
            Expr::Ident(ident) => ident.sym.as_ref(),
            _ => return None,
        };
        Some((base, property_name(&member.prop)?))
    }
    fn prototype_member_name<'a>(&self, member: &'a MemberExpr) -> Option<(&'a str, &'a str)> {
        let Expr::Member(parent) = &*member.obj else {
            return None;
        };
        let (base, property) = self.member_name(parent)?;
        if property != "prototype" {
            return None;
        }
        Some((base, property_name(&member.prop)?))
    }
    fn inspect_member(&mut self, member: &MemberExpr) {
        if let Some((base, property)) = self.member_name(member) {
            if self.cloudfront_bindings.contains(base) && property != "kvs" {
                self.add(
                    member.span,
                    "CFF012",
                    format!("cloudfront.{property} is not supported by this version of cff-test"),
                    "use cloudfront.kvs() only",
                );
            }
            if base == "console" && property != "log" {
                self.add(
                    member.span,
                    "CFF007",
                    "console only supports console.log()",
                    "write diagnostics with console.log(value)",
                );
            }
            if base == "globalThis" && matches!(property, "eval" | "Function") {
                self.add(
                    member.span,
                    "CFF003",
                    format!(
                        "dynamic code evaluation through globalThis.{property} is not supported"
                    ),
                    "remove eval() and Function constructors",
                );
            }
            if base == "globalThis" && runtime20::UNSUPPORTED_GLOBALS.contains(&property) {
                let rule = if runtime20::TIMER_GLOBALS.contains(&property) {
                    "CFF005"
                } else {
                    "CFF004"
                };
                self.add(
                    member.span,
                    rule,
                    format!("unsupported global '{property}'"),
                    "remove the restricted global API",
                );
            }
            if base != "console"
                && !self.bound(base)
                && runtime20::STATIC_MEMBERS
                    .iter()
                    .any(|(name, _)| *name == base)
                && !runtime20::static_member_allowed(base, property)
            {
                self.add(
                    member.span,
                    "CFF006",
                    format!("unsupported member {base}.{property}"),
                    "use a member listed for CloudFront Functions runtime 2.0",
                );
            }
        }
        if let Some((base, property)) = self.prototype_member_name(member)
            && !self.bound(base)
            && runtime20::PROTOTYPE_MEMBERS
                .iter()
                .any(|(name, _)| *name == base)
            && !runtime20::prototype_member_allowed(base, property)
        {
            self.add(
                member.span,
                "CFF006",
                format!("unsupported member {base}.prototype.{property}"),
                "use a member listed for CloudFront Functions runtime 2.0",
            );
        }
    }
}

fn property_name(property: &MemberProp) -> Option<&str> {
    match property {
        MemberProp::Ident(ident) => Some(ident.sym.as_ref()),
        MemberProp::Computed(computed) => match computed.expr.as_lit() {
            Some(Lit::Str(value)) => value.value.as_wtf8().as_str(),
            _ => None,
        },
        MemberProp::PrivateName(_) => None,
    }
}

impl Visit for RuleVisitor<'_> {
    fn visit_ident(&mut self, node: &Ident) {
        let name = node.sym.to_string();
        if runtime20::UNSUPPORTED_GLOBALS.contains(&name.as_str()) && !self.bound(&name) {
            let rule = if runtime20::TIMER_GLOBALS.contains(&name.as_str()) {
                "CFF005"
            } else {
                "CFF004"
            };
            self.add(
                node.span,
                rule,
                format!("unsupported global '{name}'"),
                "remove the restricted API from the function",
            );
        }
    }
    fn visit_member_expr(&mut self, node: &MemberExpr) {
        self.inspect_member(node);
        node.obj.visit_with(self);
        if let MemberProp::Computed(prop) = &node.prop {
            prop.expr.visit_with(self);
        }
    }
    fn visit_call_expr(&mut self, node: &CallExpr) {
        if matches!(&node.callee, Callee::Import(_)) {
            self.add(
                node.span,
                "CFF008",
                "dynamic import is not supported by CloudFront Functions runtime 2.0",
                "import only crypto, querystring, or cloudfront as a static module",
            );
        }
        if let Callee::Expr(callee) = &node.callee {
            if let Expr::Ident(ident) = &**callee {
                let name = ident.sym.to_string();
                if matches!(name.as_str(), "eval" | "Function")
                    || self.forbidden_aliases.contains(&name)
                {
                    self.add(
                        node.span,
                        "CFF003",
                        format!("dynamic code evaluation through {name} is not supported"),
                        "remove eval() and Function constructors",
                    );
                }
                if name == "require" {
                    check_require(self, node);
                }
            }
            if let Expr::Member(member) = &**callee
                && self
                    .member_name(member)
                    .or_else(|| property_name(&member.prop).map(|property| ("<value>", property)))
                    .is_some_and(|(_, property)| property == "constructor")
            {
                self.add(
                    node.span,
                    "CFF003",
                    "Function constructor is not supported",
                    "remove Function.prototype.constructor calls",
                );
            }
            if let Expr::Member(member) = &**callee
                && self
                    .member_name(member)
                    .is_some_and(|(base, property)| base == "console" && property == "log")
                && node.args.len() != 1
            {
                self.add(
                    node.span,
                    "CFF007",
                    "console.log() requires exactly one argument",
                    "combine values into one argument",
                );
            }
        }
        node.callee.visit_with(self);
        for arg in &node.args {
            arg.visit_with(self);
        }
    }
    fn visit_new_expr(&mut self, node: &NewExpr) {
        if matches!(&*node.callee, Expr::Ident(ident) if ident.sym == "Function") {
            self.add(
                node.span(),
                "CFF003",
                "Function constructor is not supported",
                "remove new Function(...)",
            );
        }
        node.callee.visit_with(self);
        node.args.visit_with(self);
    }
    fn visit_lit(&mut self, node: &Lit) {
        if matches!(node, Lit::BigInt(_)) {
            self.add(
                node.span(),
                "CFF002",
                "BigInt literals are not supported by CloudFront runtime 2.0",
                "use a supported Number representation",
            );
        }
    }
    fn visit_var_declarator(&mut self, node: &VarDeclarator) {
        if let Some(init) = &node.init {
            if let Pat::Ident(binding) = &node.name {
                if matches!(&**init, Expr::Ident(ident) if ident.sym == "eval" || ident.sym == "Function")
                {
                    self.forbidden_aliases.insert(binding.id.sym.to_string());
                }
                if let Expr::Call(call) = &**init
                    && let Callee::Expr(callee) = &call.callee
                    && matches!(&**callee, Expr::Ident(ident) if ident.sym == "require")
                    && let Some(argument) = call.args.first()
                    && let Expr::Lit(Lit::Str(module)) = &*argument.expr
                    && module.value.as_wtf8().as_str() == Some("cloudfront")
                {
                    self.cloudfront_bindings.insert(binding.id.sym.to_string());
                }
            }
            init.visit_with(self);
        }
    }
    fn visit_function(&mut self, node: &Function) {
        if node.is_generator && node.is_async {
            self.add(
                node.span,
                "CFF011",
                "async generators are not supported",
                "use a synchronous generator or an async function",
            );
        }
        if node.is_async {
            self.async_depth += 1;
        }
        node.params.visit_with(self);
        node.body.visit_with(self);
        if node.is_async {
            self.async_depth -= 1;
        }
    }
    fn visit_arrow_expr(&mut self, node: &ArrowExpr) {
        if node.is_async {
            self.add(
                node.span,
                "CFF011",
                "async arrow functions are not supported",
                "use an async function declaration only",
            );
        }
        node.params.visit_with(self);
        if node.is_async {
            self.async_depth += 1;
        }
        node.body.visit_with(self);
        if node.is_async {
            self.async_depth -= 1;
        }
    }
    fn visit_fn_expr(&mut self, node: &FnExpr) {
        if node.function.is_async {
            self.add(
                node.span(),
                "CFF011",
                "async function expressions are not supported",
                "use an async function declaration only",
            );
        }
        node.function.visit_with(self);
    }
    fn visit_class_method(&mut self, node: &ClassMethod) {
        if node.function.is_async {
            self.add(
                node.span,
                "CFF011",
                "async methods are not supported",
                "use an async function declaration only",
            );
        }
        node.function.visit_with(self);
    }
    fn visit_await_expr(&mut self, node: &AwaitExpr) {
        if self.async_depth == 0 {
            self.add(
                node.span,
                "CFF011",
                "await is only allowed in an async function",
                "move await inside an async function",
            );
        }
        node.arg.visit_with(self);
    }
    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.add(
            node.class.span,
            "CFF002",
            "class syntax is not supported by CloudFront runtime 2.0",
            "use ES 5.1 constructor functions",
        );
        node.class.visit_with(self);
    }
    fn visit_for_of_stmt(&mut self, node: &ForOfStmt) {
        self.add(
            node.span,
            "CFF002",
            "for...of syntax is not supported by CloudFront runtime 2.0",
            "use a supported loop form",
        );
        node.left.visit_with(self);
        node.right.visit_with(self);
        node.body.visit_with(self);
    }
    fn visit_opt_chain_expr(&mut self, node: &OptChainExpr) {
        self.add(
            node.span,
            "CFF002",
            "optional chaining is not supported by CloudFront runtime 2.0",
            "use an explicit null check",
        );
        node.base.visit_with(self);
    }
    fn visit_bin_expr(&mut self, node: &BinExpr) {
        if node.op == BinaryOp::NullishCoalescing {
            self.add(
                node.span,
                "CFF002",
                "nullish coalescing is not supported by CloudFront runtime 2.0",
                "use an explicit null check",
            );
        }
        node.left.visit_with(self);
        node.right.visit_with(self);
    }
}

fn check_require(visitor: &mut RuleVisitor<'_>, node: &CallExpr) {
    let valid = node.args.len() == 1
        && matches!(&*node.args[0].expr, Expr::Lit(Lit::Str(module)) if module.value.as_wtf8().as_str().is_some_and(|name| runtime20::SUPPORTED_MODULES.contains(&name)));
    if !valid {
        visitor.add(
            node.span,
            "CFF008",
            "require() may load only crypto, querystring, or cloudfront",
            "use require('crypto'), require('querystring'), or require('cloudfront')",
        );
    }
}

fn check_handler(path: &Path, source: &str, module: &Module, diagnostics: &mut Vec<Diagnostic>) {
    let mut handlers = Vec::new();
    for item in &module.body {
        if let ModuleItem::Stmt(Stmt::Decl(Decl::Fn(function))) = item
            && function.ident.sym == "handler"
        {
            handlers.push((
                function.span(),
                function.function.params.len(),
                function.function.is_generator,
            ));
        }
        if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(variable))) = item {
            for declarator in &variable.decls {
                if let Pat::Ident(binding) = &declarator.name
                    && binding.id.sym == "handler"
                {
                    if let Some(Expr::Fn(function)) = declarator.init.as_deref() {
                        handlers.push((
                            declarator.span(),
                            function.function.params.len(),
                            function.function.is_generator,
                        ));
                    } else if let Some(Expr::Arrow(arrow)) = declarator.init.as_deref() {
                        handlers.push((declarator.span(), arrow.params.len(), false));
                    }
                }
            }
        }
    }
    match handlers.as_slice() {
        [] => diagnostics.push(make_diagnostic(
            path,
            source,
            module.span,
            "CFF009",
            "top-level handler(event) was not found",
            "declare exactly one top-level handler with one argument",
        )),
        [(_, params, generator)] if *params == 1 && !generator => {}
        _ => diagnostics.push(make_diagnostic(
            path,
            source,
            handlers[0].0,
            "CFF009",
            "top-level handler must be declared exactly once with one argument",
            "declare handler(event) or async handler(event)",
        )),
    }
}

fn make_diagnostic(
    path: &Path,
    source: &str,
    span: Span,
    rule: &'static str,
    message: &str,
    help: &str,
) -> Diagnostic {
    let byte_start = span.lo.0.saturating_sub(1) as usize;
    let prefix = &source[..byte_start.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix, |(_, line)| line)
        .chars()
        .count()
        + 1;
    Diagnostic {
        path: path.to_path_buf(),
        line,
        column,
        rule,
        message: message.into(),
        help: help.into(),
        byte_start,
    }
}
