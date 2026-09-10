//! Source recovery through the vendored dexdec pipeline.
//!
//! Every entry point returns `None` on any failure; callers fall back to the
//! structural-IR emitter. Language handling follows the DECX contract:
//! `language` = `java` (default) | `kotlin` | `auto`.

use crate::project::Project;
use decx_engine::{MethodRequest, SourceLanguage};

/// (source, mode, language) where mode is "dexdec" and language "java"/"kotlin"
pub type Recovered = (String, &'static str, &'static str);

fn lang_of(sl: SourceLanguage) -> &'static str {
    match sl {
        SourceLanguage::Kotlin => "kotlin",
        SourceLanguage::Java => "java",
    }
}

fn configure(dec: &mut decx_engine::Decompiler, language: SourceLanguage) {
    let mut opts = dec.options().clone();
    opts.language = language;
    dec.set_options(opts);
}

fn resolve_language(dec: &mut decx_engine::Decompiler, class: &str, request: &str) -> SourceLanguage {
    match request {
        "kotlin" => SourceLanguage::Kotlin,
        "auto" => dec.source_language(class).unwrap_or(SourceLanguage::Java),
        _ => SourceLanguage::Java,
    }
}

/// dexdec's class table is keyed by the raw DEX descriptor (`Lcom/foo/Bar;`).
/// Accept dotted names (`com.foo.Bar`) — what our routes and the DECX
/// contract use — and also already-descriptor input.
fn to_descriptor(class: &str) -> String {
    if class.starts_with('L') && class.ends_with(';') && class.contains('/') {
        return class.to_string();
    }
    format!("L{};", class.replace('.', "/"))
}

/// Recover one full compilation unit through dexdec.
pub fn try_dexdec_class(p: &Project, class_dotted: &str, language_request: &str) -> Option<Recovered> {
    let mut guard = p.decomp();
    let dec = guard.as_mut()?;
    let descriptor = to_descriptor(class_dotted);
    let language = resolve_language(dec, &descriptor, language_request);
    configure(dec, language);
    let unit = dec.class(descriptor).ok()?;
    Some((unit.source, "dexdec", lang_of(unit.language)))
}

/// Recover one method body through dexdec.
/// `descriptor` is `(args)ret` when known, `None` lets dexdec disambiguate.
pub fn try_dexdec_method(
    p: &Project,
    owner_dotted: &str,
    method_name: &str,
    descriptor: Option<String>,
    language_request: &str,
) -> Option<Recovered> {
    let mut guard = p.decomp();
    let dec = guard.as_mut()?;
    let class_descriptor = to_descriptor(owner_dotted);
    let language = resolve_language(dec, &class_descriptor, language_request);
    configure(dec, language);
    let mut request = MethodRequest::new(class_descriptor, method_name.to_string());
    request.descriptor = descriptor;
    let output = dec.method(request).ok()?;
    output.source.map(|src| (src, "dexdec", lang_of(output.language)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_mapping() {
        assert_eq!(lang_of(SourceLanguage::Java), "java");
        assert_eq!(lang_of(SourceLanguage::Kotlin), "kotlin");
    }

    #[test]
    fn descriptor_forms() {
        assert_eq!(to_descriptor("com.vivo.weather.WeatherMain"), "Lcom/vivo/weather/WeatherMain;");
        assert_eq!(
            to_descriptor("Lcom/vivo/weather/WeatherMain;"),
            "Lcom/vivo/weather/WeatherMain;"
        );
    }
}
