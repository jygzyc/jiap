//! debug helper: dump decoded AndroidManifest.xml of an apk
use decx_apk::Apk;

fn main() {
    let path = std::env::args().nth(1).expect("apk path");
    let data = std::fs::read(&path).unwrap();
    let apk = Apk::open(data).unwrap();
    match apk.manifest_xml {
        Some(xml) => {
            println!("{xml}");
            if let Some(m) = apk.manifest {
                eprintln!("--- package={} comps={} perms={}", m.package, m.components.len(), m.permissions.len());
                for c in m.components.iter().take(5) {
                    eprintln!("    {} {} actions={:?}", c.kind, c.name, c.intent_actions);
                }
                eprintln!("main_activity={:?}", m.main_activity().map(|c| c.name.clone()));
            }
        }
        None => eprintln!("no manifest"),
    }
}
