use std::path::{Path, PathBuf};

pub fn lexically_normal_path(input: &Path) -> Option<PathBuf> {
    let mut result = PathBuf::new();
    for component in input.components() {
        match component {
            std::path::Component::Prefix(_) => return None,
            std::path::Component::RootDir => return None,
            std::path::Component::CurDir => (),
            std::path::Component::ParentDir => {
                // Pop something from our buffer if we can; if not, it's not a safe relative path.
                if !result.pop() {
                    return None;
                }
            },
            std::path::Component::Normal(part) => {
                result.push(part);
            },
        }
    }
    Some(result)
}

#[cfg(test)]
mod test {
    use std::assert_eq;

    use super::*;

    #[test]
    fn leaves_good_paths_alone() {
        // Test a file
        assert_eq!(
            lexically_normal_path(Path::new("this/is/fine")),
            Some(Path::new("this/is/fine").to_path_buf())
        );
        // Test a folder
        assert_eq!(
            lexically_normal_path(Path::new("this/is/fine/")),
            Some(Path::new("this/is/fine/").to_path_buf())
        );
    }

    #[test]
    fn simplifies_paths() {
        // Test a file
        assert_eq!(
            lexically_normal_path(Path::new("this/../fine")),
            Some(Path::new("fine").to_path_buf())
        );
        // Test a folder
        assert_eq!(
            lexically_normal_path(Path::new("this/./fine/")),
            Some(Path::new("this/fine/").to_path_buf())
        );
    }

    #[test]
    fn can_handle_empty_paths() {
        // Explicit
        assert_eq!(
            lexically_normal_path(Path::new(".")),
            Some(Path::new("").to_path_buf())
        );
        // Empty
        assert_eq!(
            lexically_normal_path(Path::new("")),
            Some(Path::new("").to_path_buf())
        );
        // Parent of top-level folder
        assert_eq!(
            lexically_normal_path(Path::new("folder/..")),
            Some(Path::new("").to_path_buf())
        );
    }

    #[test]
    fn rejects_absolute_paths() {
        assert_eq!(
            lexically_normal_path(Path::new("/hello")),
            None
        );
    }

    #[test]
    fn rejects_paths_that_leave_directory() {
        assert_eq!(
            lexically_normal_path(Path::new("..")),
            None
        );
        assert_eq!(
            lexically_normal_path(Path::new("/hello/../../what/is/up")),
            None
        );
    }
}