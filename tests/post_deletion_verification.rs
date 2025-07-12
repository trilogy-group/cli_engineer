#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn test_unauthorized_files_deleted() -> std::io::Result<()> {
        let unauthorized_files = vec![
            "verify_test_files_deletion.rs",
            "file_verification.rs",
            "config_analysis.rs",
            "file_deletion.rs",
            "event_bus.rs",
            "llm_manager.rs",
        ];

        for file in unauthorized_files {
            let path = Path::new("tests").join(file);
            assert!(!path.exists(), "Unauthorized file {} still exists", file);
        }

        Ok(())
    }

    #[test]
    fn test_directory_is_clean() -> std::io::Result<()> {
        let dir = std::fs::read_dir("tests")?;
        let file_count = dir.count();
        assert_eq!(file_count, 0, "Tests directory should be empty after deletions, but contains {} files", file_count);
        Ok(())
    }
}