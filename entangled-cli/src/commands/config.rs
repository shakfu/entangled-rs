//! Config command implementation.

use entangled::errors::Result;
use entangled::interface::Context;

/// Executes the config command -- prints the effective resolved configuration.
pub fn config(ctx: &Context) -> Result<()> {
    let toml_str = toml::to_string_pretty(&ctx.config).map_err(|e| {
        entangled::EntangledError::Other(format!("Failed to serialize config: {}", e))
    })?;
    print!("{}", toml_str);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_prints() {
        let dir = tempdir().unwrap();
        let ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        config(&ctx).unwrap();
    }
}
