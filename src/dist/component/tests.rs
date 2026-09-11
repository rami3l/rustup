use std::{fs, io::Write, path::PathBuf};

use crate::{errors::RustupError, test::DistContext, utils};

#[test]
fn add_file() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let mut file = tx.add_file("c", PathBuf::from("foo/bar"))?;
    write!(file, "test")?;

    tx.commit()?;
    drop(file);

    assert_eq!(
        fs::read_to_string(cx.prefix.path().join("foo/bar"))?,
        "test"
    );
    Ok(())
}

#[test]
fn add_file_then_rollback() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    tx.add_file("c", PathBuf::from("foo/bar"))?;
    drop(tx);

    assert!(!utils::is_file(cx.prefix.path().join("foo/bar")));
    Ok(())
}

#[test]
fn add_file_that_exists() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    fs::create_dir_all(cx.prefix.path().join("foo"))?;
    utils::write_file("", &cx.prefix.path().join("foo/bar"), "")?;

    tx.add_file("c", PathBuf::from("foo/bar"))?;
    Ok(())
}

#[test]
fn copy_file() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let srcpath = cx.pkg_dir.path().join("bar");
    utils::write_file("", &srcpath, "")?;

    tx.copy_file("c", PathBuf::from("foo/bar"), &srcpath)?;
    tx.commit()?;

    assert!(utils::is_file(cx.prefix.path().join("foo/bar")));
    Ok(())
}

#[test]
fn copy_file_then_rollback() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let srcpath = cx.pkg_dir.path().join("bar");
    utils::write_file("", &srcpath, "")?;

    tx.copy_file("c", PathBuf::from("foo/bar"), &srcpath)?;
    drop(tx);

    assert!(!utils::is_file(cx.prefix.path().join("foo/bar")));
    Ok(())
}

#[test]
fn copy_file_that_exists() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let srcpath = cx.pkg_dir.path().join("bar");
    utils::write_file("", &srcpath, "")?;

    fs::create_dir_all(cx.prefix.path().join("foo"))?;
    utils::write_file("", &cx.prefix.path().join("foo/bar"), "")?;

    tx.copy_file("c", PathBuf::from("foo/bar"), &srcpath)?;
    Ok(())
}

#[test]
fn copy_dir() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let srcpath1 = cx.pkg_dir.path().join("foo");
    let srcpath2 = cx.pkg_dir.path().join("bar/baz");
    let srcpath3 = cx.pkg_dir.path().join("bar/qux/tickle");
    utils::write_file("", &srcpath1, "")?;
    fs::create_dir_all(srcpath2.parent().unwrap())?;
    utils::write_file("", &srcpath2, "")?;
    fs::create_dir_all(srcpath3.parent().unwrap())?;
    utils::write_file("", &srcpath3, "")?;

    tx.copy_dir("c", PathBuf::from("a"), cx.pkg_dir.path())?;
    tx.commit()?;

    assert!(utils::is_file(cx.prefix.path().join("a/foo")));
    assert!(utils::is_file(cx.prefix.path().join("a/bar/baz")));
    assert!(utils::is_file(cx.prefix.path().join("a/bar/qux/tickle")));

    Ok(())
}

#[test]
fn copy_dir_then_rollback() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let srcpath1 = cx.pkg_dir.path().join("foo");
    let srcpath2 = cx.pkg_dir.path().join("bar/baz");
    let srcpath3 = cx.pkg_dir.path().join("bar/qux/tickle");
    utils::write_file("", &srcpath1, "")?;
    fs::create_dir_all(srcpath2.parent().unwrap())?;
    utils::write_file("", &srcpath2, "")?;
    fs::create_dir_all(srcpath3.parent().unwrap())?;
    utils::write_file("", &srcpath3, "")?;

    tx.copy_dir("c", PathBuf::from("a"), cx.pkg_dir.path())?;
    drop(tx);

    assert!(!utils::is_file(cx.prefix.path().join("a/foo")));
    assert!(!utils::is_file(cx.prefix.path().join("a/bar/baz")));
    assert!(!utils::is_file(cx.prefix.path().join("a/bar/qux/tickle")));

    Ok(())
}

#[test]
fn copy_dir_that_exists() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;
    fs::create_dir_all(tx.dest_abs_path(&PathBuf::from("a"))?)?;

    assert!(
        tx.copy_dir("c", PathBuf::from("a"), cx.pkg_dir.path())
            .is_err()
    );

    Ok(())
}

#[test]
fn remove_file() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;
    let filepath = cx.prefix.path().join("foo");
    let temp_path = tx.dest_abs_path(&PathBuf::from("foo"))?;
    utils::write_file("", &temp_path, "")?;

    tx.remove_file("c", PathBuf::from("foo"))?;
    tx.commit()?;

    assert!(!utils::is_file(filepath));

    Ok(())
}

#[test]
fn remove_file_then_rollback() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;
    let filepath = cx.prefix.path().join("foo");
    let temp_path = tx.dest_abs_path(&PathBuf::from("foo"))?;
    utils::write_file("", &temp_path, "")?;

    tx.remove_file("c", PathBuf::from("foo"))?;
    drop(tx);

    assert!(!utils::is_file(filepath));

    Ok(())
}

#[test]
fn remove_file_that_not_exists() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let err = tx.remove_file("c", PathBuf::from("foo")).unwrap_err();

    match err.downcast_ref::<RustupError>() {
        Some(RustupError::ComponentMissingFile { name, path }) => {
            assert_eq!(name, "c");
            assert_eq!(path.clone(), PathBuf::from("foo"));
        }
        _ => panic!(),
    }

    Ok(())
}

#[test]
fn remove_dir() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;
    let filepath = cx.prefix.path().join("foo/bar");
    let temp_path = tx.dest_abs_path(&PathBuf::from("foo/bar"))?;
    utils::write_file("", &temp_path, "")?;

    tx.remove_dir("c", PathBuf::from("foo"))?;
    tx.commit()?;

    assert!(!utils::path_exists(filepath.parent().unwrap()));

    Ok(())
}

#[test]
fn remove_dir_then_rollback() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;
    let filepath = cx.prefix.path().join("foo/bar");
    let temp_path = tx.dest_abs_path(&PathBuf::from("foo/bar"))?;
    utils::write_file("", &temp_path, "")?;

    tx.remove_dir("c", PathBuf::from("foo"))?;
    drop(tx);

    assert!(!utils::path_exists(filepath.parent().unwrap()));

    Ok(())
}

#[test]
fn remove_dir_that_not_exists() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let err = tx.remove_dir("c", PathBuf::from("foo")).unwrap_err();

    match err.downcast_ref::<RustupError>() {
        Some(RustupError::ComponentMissingDir { name, path }) => {
            assert_eq!(name, "c");
            assert_eq!(path.clone(), PathBuf::from("foo"));
        }
        _ => panic!(),
    }

    Ok(())
}

#[test]
fn write_file() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let content = "hi".to_string();
    tx.write_file("c", PathBuf::from("foo/bar"), content.clone())?;
    tx.commit()?;

    let path = cx.prefix.path().join("foo/bar");
    assert!(utils::is_file(&path));
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, file_content);

    Ok(())
}

#[test]
fn write_file_then_rollback() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let content = "hi".to_string();
    tx.write_file("c", PathBuf::from("foo/bar"), content)?;
    drop(tx);

    assert!(!utils::is_file(cx.prefix.path().join("foo/bar")));

    Ok(())
}

#[test]
fn write_file_that_exists() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let content = "hi".to_string();
    let mut tx = cx.transaction()?;
    let temp_path = tx.dest_abs_path(&PathBuf::from("a"))?;
    utils::write_file("", &temp_path, "old")?;
    tx.write_file("c", PathBuf::from("a"), content.clone())?;
    tx.commit()?;
    assert_eq!(fs::read_to_string(cx.prefix.path().join("a"))?, content);

    Ok(())
}

fn do_multiple_op_transaction(rollback: bool) -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;

    // copy_file
    let relpath1 = PathBuf::from("bin/rustc");
    let relpath2 = PathBuf::from("bin/cargo");
    // copy_dir
    let relpath4 = PathBuf::from("doc/html/index.html");
    // modify_file
    let relpath5 = PathBuf::from("lib/rustlib/components");
    // write_file
    let relpath6 = PathBuf::from("lib/rustlib/rustc-manifest.in");
    // remove_file
    let relpath7 = PathBuf::from("bin/oldrustc");
    // remove_dir
    let relpath8 = PathBuf::from("olddoc/htm/index.html");

    let path1 = cx.prefix.path().join(&relpath1);
    let path2 = cx.prefix.path().join(&relpath2);
    let path4 = cx.prefix.path().join(&relpath4);
    let path5 = cx.prefix.path().join(&relpath5);
    let path6 = cx.prefix.path().join(&relpath6);
    let path7 = cx.prefix.path().join(&relpath7);
    let path8 = cx.prefix.path().join(&relpath8);

    let mut tx = cx.transaction()?;
    let temp_path7 = tx.dest_abs_path(&relpath7)?;
    utils::write_file("", &temp_path7, "")?;
    let temp_path8 = tx.dest_abs_path(&relpath8)?;
    utils::write_file("", &temp_path8, "")?;

    let srcpath1 = cx.pkg_dir.path().join(&relpath1);
    fs::create_dir_all(srcpath1.parent().unwrap())?;
    utils::write_file("", &srcpath1, "")?;
    tx.copy_file("", relpath1, &srcpath1)?;

    let srcpath2 = cx.pkg_dir.path().join(&relpath2);
    utils::write_file("", &srcpath2, "")?;
    tx.copy_file("", relpath2, &srcpath2)?;

    let srcpath4 = cx.pkg_dir.path().join(&relpath4);
    fs::create_dir_all(srcpath4.parent().unwrap())?;
    utils::write_file("", &srcpath4, "")?;
    tx.copy_dir("", PathBuf::from("doc"), &cx.pkg_dir.path().join("doc"))?;

    let temp_path5 = tx.dest_abs_path(&relpath5)?;
    utils::write_file("", &temp_path5, "")?;

    tx.write_file("", relpath6, "".to_string())?;

    tx.remove_file("", relpath7)?;

    tx.remove_dir("", PathBuf::from("olddoc"))?;

    if !rollback {
        tx.commit()?;

        assert!(utils::path_exists(path1));
        assert!(utils::path_exists(path2));
        assert!(utils::path_exists(path4));
        assert!(utils::path_exists(path5));
        assert!(utils::path_exists(path6));
        assert!(!utils::path_exists(path7));
        assert!(!utils::path_exists(path8));
    } else {
        drop(tx);

        assert!(!utils::path_exists(path1));
        assert!(!utils::path_exists(path2));
        assert!(!utils::path_exists(path4));
        assert!(!utils::path_exists(path5));
        assert!(!utils::path_exists(path6));
        assert!(!utils::path_exists(path7));
        assert!(!utils::path_exists(path8));
    }
    Ok(())
}

#[test]
fn multiple_op_transaction() -> anyhow::Result<()> {
    do_multiple_op_transaction(false)?;
    Ok(())
}

#[test]
fn multiple_op_transaction_then_rollback() -> anyhow::Result<()> {
    do_multiple_op_transaction(true)?;
    Ok(())
}

// Even if one step fails to rollback, rollback should
// continue to rollback other steps.
#[test]
fn rollback_failure_keeps_going() -> anyhow::Result<()> {
    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    write!(tx.add_file("", PathBuf::from("foo"))?, "")?;
    write!(tx.add_file("", PathBuf::from("bar"))?, "")?;
    write!(tx.add_file("", PathBuf::from("baz"))?, "")?;

    fs::remove_file(tx.dest_abs_path(&PathBuf::from("bar"))?)?;

    drop(tx);

    assert!(!utils::path_exists(cx.prefix.path().join("foo")));
    assert!(!utils::path_exists(cx.prefix.path().join("baz")));

    Ok(())
}

// Test that when a transaction creates intermediate directories that
// they are deleted during rollback.
#[test]
#[ignore]
fn intermediate_dir_rollback() {}

#[test]
#[cfg(unix)]
fn copy_dir_preserves_symlinks() -> anyhow::Result<()> {
    // copy_dir must preserve symlinks, not follow them
    use std::os::unix::fs::symlink;

    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let src_dir = cx.pkg_dir.path();

    let src_real_file = src_dir.join("real_file.txt");
    utils::write_file("", &src_real_file, "original content")?;

    let src_subdir = src_dir.join("subdir");
    fs::create_dir(&src_subdir)?;

    let src_subdir_link_to_file = src_subdir.join("link_to_file.txt");
    symlink("../real_file.txt", &src_subdir_link_to_file).unwrap();

    let src_real_dir = src_dir.join("real_dir");
    fs::create_dir(&src_real_dir)?;
    utils::write_file("", &src_real_dir.join("inner.txt"), "inner content")?;
    let src_subdir_link_to_dir = src_subdir.join("link_to_dir");
    symlink("../real_dir", &src_subdir_link_to_dir).unwrap();

    assert!(
        fs::symlink_metadata(&src_subdir_link_to_file)
            .unwrap()
            .file_type()
            .is_symlink(),
        "Source file symlink should be a symlink"
    );
    assert!(
        fs::symlink_metadata(&src_subdir_link_to_dir)
            .unwrap()
            .file_type()
            .is_symlink(),
        "Source dir symlink should be a symlink"
    );

    tx.copy_dir("test-component", PathBuf::from("dest"), src_dir)?;
    tx.commit()?;

    let dest_file_symlink = cx.prefix.path().join("dest/subdir/link_to_file.txt");
    let dest_dir_symlink = cx.prefix.path().join("dest/subdir/link_to_dir");

    assert!(
        fs::symlink_metadata(&dest_file_symlink)
            .unwrap()
            .file_type()
            .is_symlink(),
        "Destination file symlink should be preserved as a symlink"
    );
    assert!(
        fs::symlink_metadata(&dest_dir_symlink)
            .unwrap()
            .file_type()
            .is_symlink(),
        "Destination dir symlink should be preserved as a symlink"
    );

    assert_eq!(
        fs::read_link(&dest_file_symlink).unwrap().to_str().unwrap(),
        "../real_file.txt",
        "File symlink target should be preserved"
    );
    assert_eq!(
        fs::read_link(&dest_dir_symlink).unwrap().to_str().unwrap(),
        "../real_dir",
        "Dir symlink target should be preserved"
    );

    Ok(())
}

/// Test that utils::copy_file preserves symlink targets
#[test]
#[cfg(unix)]
fn copy_file_preserves_symlinks() -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let src_dir = tmp.path().join("src");
    let dest_dir = tmp.path().join("dest");
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&dest_dir)?;

    let src_real_file = src_dir.join("real_file.txt");
    utils::write_file("", &src_real_file, "content")?;

    let src_link_file = src_dir.join("link.txt");
    symlink("real_file.txt", &src_link_file).unwrap();

    assert!(
        fs::symlink_metadata(&src_link_file)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_link(&src_link_file).unwrap().to_str().unwrap(),
        "real_file.txt"
    );

    // copy_file should preserve the symlink target
    let dest_link_file = dest_dir.join("link.txt");
    utils::copy_file(&src_link_file, &dest_link_file).unwrap();

    assert!(
        fs::symlink_metadata(&dest_link_file)
            .unwrap()
            .file_type()
            .is_symlink(),
        "copy_file should preserve symlinks"
    );
    assert_eq!(
        fs::read_link(&dest_link_file).unwrap().to_str().unwrap(),
        "real_file.txt",
        "copy_file should preserve the original symlink target"
    );

    Ok(())
}

/// Test that utils::copy_file_symlink_to_source creates a symlink pointing to the source path
#[test]
#[cfg(unix)]
fn copy_file_symlink_to_source_creates_symlink_to_source() -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let src_dir = tmp.path().join("src");
    let dest_dir = tmp.path().join("dest");
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&dest_dir)?;

    let src_real_file = src_dir.join("real_file.txt");
    utils::write_file("", &src_real_file, "original content")?;

    let src_link_file = src_dir.join("link.txt");
    symlink("real_file.txt", &src_link_file).unwrap();

    assert!(
        fs::symlink_metadata(&src_link_file)
            .unwrap()
            .file_type()
            .is_symlink()
    );

    // copy_file_symlink_to_source should create a symlink pointing to the source path
    let dest_link_file = dest_dir.join("copied.txt");
    utils::copy_file_symlink_to_source(&src_link_file, &dest_link_file).unwrap();

    // Destination should be a symlink pointing to the source path
    assert!(
        fs::symlink_metadata(&dest_link_file)
            .unwrap()
            .file_type()
            .is_symlink(),
        "copy_file_symlink_to_source should create a symlink"
    );
    assert_eq!(
        fs::read_link(&dest_link_file).unwrap(),
        src_link_file,
        "copy_file_symlink_to_source should create a symlink pointing to the source path"
    );

    Ok(())
}

/// Test that Transaction::copy_file (which uses utils::copy_file) preserves symlinks
#[test]
#[cfg(unix)]
fn transaction_copy_file_preserves_symlinks() -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let cx = DistContext::new(None)?;
    let mut tx = cx.transaction()?;

    let src_dir = cx.pkg_dir.path();
    let real_file = src_dir.join("real_file.txt");
    utils::write_file("", &real_file, "content")?;

    let link_file = src_dir.join("link.txt");
    symlink("real_file.txt", &link_file).unwrap();

    tx.copy_file(
        "test-component",
        PathBuf::from("copied_link.txt"),
        &link_file,
    )
    .unwrap();
    tx.commit()?;

    let dest_link = cx.prefix.path().join("copied_link.txt");
    assert!(
        fs::symlink_metadata(&dest_link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "Transaction::copy_file should preserve symlinks"
    );
    assert_eq!(
        fs::read_link(&dest_link).unwrap().to_str().unwrap(),
        "real_file.txt",
        "Transaction::copy_file should preserve symlink target"
    );

    Ok(())
}
