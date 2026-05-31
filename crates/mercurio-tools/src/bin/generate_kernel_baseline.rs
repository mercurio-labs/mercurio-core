use std::fs;

use mercurio_core::{KirDocument, default_kernel_library_path};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = default_kernel_library_path();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let document = json!({
        "metadata": {
            "library_id": "org.omg/kerml-kernel",
            "library_version": "0.0.0-bootstrap",
            "generator": "cargo run -p mercurio-tools --bin generate_kernel_baseline",
            "note": "Bootstrap KerML kernel baseline. This intentionally contains no elements until a generated Kernel/KerML library artifact is wired in."
        },
        "elements": []
    });

    fs::write(
        &output_path,
        serde_json::to_string_pretty(&document)? + "\n",
    )?;
    KirDocument::from_path(&output_path)?;

    println!("wrote {}", output_path.display());
    Ok(())
}
