use crate::Error;
use std::ops::Deref;

#[derive(Clone, Debug)]
pub struct Program {
    core: galileo_core::Program,
    pub(crate) identity: galileo_review::Identity,
    sources: Vec<(String, String)>,
}

impl Program {
    pub fn load(bytes: &[u8]) -> Result<Self, Error> {
        let mut remaining = bytes;
        let header = line(&mut remaining)?;
        let identity = header
            .strip_prefix("galileo-image 1 ")
            .filter(|hex| hex.len() == 16 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .and_then(|hex| u64::from_str_radix(hex, 16).ok())
            .ok_or_else(|| Error::Image("expected galileo-image 1 <identity hex>".into()))?;
        let mut modules = Vec::new();
        while !remaining.is_empty() {
            let header = line(&mut remaining)?;
            let (name, length) = header
                .strip_prefix("module ")
                .and_then(|header| header.rsplit_once(' '))
                .ok_or_else(|| Error::Image("expected module <name> <byte length>".into()))?;
            if name.is_empty() || name.chars().any(char::is_whitespace) {
                return Err(Error::Image("expected one module name".into()));
            }
            if modules.iter().any(|(previous, _)| *previous == name) {
                return Err(Error::Image(format!("duplicate module {name}")));
            }
            let length = length
                .parse::<usize>()
                .map_err(|_| Error::Image("expected a module byte length".into()))?;
            let source = remaining
                .get(..length)
                .ok_or_else(|| Error::Image(format!("truncated source for module {name}")))?;
            let source = std::str::from_utf8(source)
                .map_err(|error| Error::Image(format!("{name}: {error}")))?;
            modules.push((name, source));
            remaining = &remaining[length..];
        }
        if modules.is_empty() {
            return Err(Error::Image("expected at least one module".into()));
        }
        let lowered = galileo_syntax::lower::lower_program(&modules)
            .map_err(|error| Error::Image(error.to_string()))?;
        if !lowered.diagnostics.is_empty() {
            return Err(Error::Image(
                lowered.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"),
            ));
        }
        let actual = galileo_review::identity(&lowered.program);
        if actual.0 != identity {
            return Err(Error::WrongProgram);
        }
        Ok(Self {
            core: lowered.program,
            identity: actual,
            sources: modules.into_iter().map(|(name, source)| (name.into(), source.into())).collect(),
        })
    }

    pub fn sources(&self) -> &[(String, String)] {
        &self.sources
    }
}

impl From<galileo_core::Program> for Program {
    fn from(core: galileo_core::Program) -> Self {
        let identity = galileo_review::identity(&core);
        Self { core, identity, sources: Vec::new() }
    }
}

impl Deref for Program {
    type Target = galileo_core::Program;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

fn line<'a>(remaining: &mut &'a [u8]) -> Result<&'a str, Error> {
    let end = remaining.iter().position(|byte| *byte == b'\n')
        .ok_or_else(|| Error::Image("expected an image header line".into()))?;
    let line = std::str::from_utf8(&remaining[..end])
        .map_err(|error| Error::Image(error.to_string()))?;
    *remaining = &remaining[end + 1..];
    Ok(line)
}
