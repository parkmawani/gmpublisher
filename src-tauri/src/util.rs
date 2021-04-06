use std::{
	collections::HashSet,
	hash::Hash,
	io::{BufRead, BufReader, BufWriter, ErrorKind, Read, Seek, SeekFrom, Write},
};

#[macro_export]
macro_rules! ignore {
	( $x:expr ) => {
		#[cfg(debug_assertions)]
		$x.unwrap();
		#[cfg(not(debug_assertions))]
		$x
	};
}

#[macro_export]
macro_rules! dprintln {
	($($x:expr),+) => {
		#[cfg(debug_assertions)]
		println!($($x),+)
	};
}

#[macro_export]
macro_rules! sleep {
	( $x:expr ) => {
		std::thread::sleep(std::time::Duration::from_secs($x))
	};
}

#[macro_export]
macro_rules! sleep_ms {
	( $x:expr ) => {
		std::thread::sleep(std::time::Duration::from_millis($x))
	};
}

#[macro_export]
macro_rules! main_thread_forbidden {
	() => {
		debug_assert_ne!(
			std::thread::current().name(),
			Some("main"),
			"This should never be called from the main thread"
		);
	};
}

#[macro_export]
macro_rules! json {
	( $x:expr ) => {
		serde_json::to_value($x).unwrap()
	};
}

#[macro_export]
macro_rules! mutex_wait {
	( $mutex:expr, $loop:block ) => {
		loop {
			if let Some(lock) = $mutex.try_lock() {
				if lock.is_some() {
					break;
				}
			}
			$loop
		}
	}
}

pub mod path {
	use serde::{de::Visitor, Deserialize, Serialize};
	use std::{fmt::Debug, path::PathBuf};

	pub fn canonicalize(path: PathBuf) -> PathBuf {
		dunce::canonicalize(path.clone()).unwrap_or(path)
	}

	#[cfg(not(target_os = "windows"))]
	pub fn normalize(path: PathBuf) -> PathBuf {
		canonicalize(path)
	}

	#[cfg(target_os = "windows")]
	pub fn normalize(path: PathBuf) -> PathBuf {
		match dunce::canonicalize(&path) {
			Ok(canonicalized) => PathBuf::from(canonicalized.to_string_lossy().to_string().replace('\\', "/")),
			Err(_) => path,
		}
	}

	#[derive(Clone)]
	pub struct NormalizedPathBuf {
		pub normalized: PathBuf,
		path: PathBuf,
	}
	impl std::ops::Deref for NormalizedPathBuf {
		type Target = PathBuf;
		fn deref(&self) -> &Self::Target {
			&self.path
		}
	}
	impl From<PathBuf> for NormalizedPathBuf {
		fn from(path: PathBuf) -> Self {
			Self {
				path: path.clone(),
				normalized: normalize(path),
			}
		}
	}
	impl From<&PathBuf> for NormalizedPathBuf {
		fn from(path: &PathBuf) -> Self {
			let path = path.to_owned();
			Self {
				path: path.clone(),
				normalized: normalize(path),
			}
		}
	}
	impl From<String> for NormalizedPathBuf {
		fn from(path: String) -> Self {
			let path = PathBuf::from(path);
			Self {
				path: path.clone(),
				normalized: normalize(path),
			}
		}
	}
	impl From<&str> for NormalizedPathBuf {
		fn from(path: &str) -> Self {
			let path = PathBuf::from(path);
			Self {
				path: path.clone(),
				normalized: normalize(path),
			}
		}
	}
	impl Debug for NormalizedPathBuf {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			self.path.fmt(f)
		}
	}

	impl Serialize for NormalizedPathBuf {
		fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where
			S: serde::Serializer,
		{
			serializer.serialize_str(&self.normalized.to_string_lossy())
		}
	}

	struct NormalizedPathBufVisitor;
	impl<'de> Visitor<'de> for NormalizedPathBufVisitor {
		type Value = String;

		fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
			formatter.write_str("a string")
		}
	}
	impl<'de> Deserialize<'de> for NormalizedPathBuf {
		fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
		where
			D: serde::Deserializer<'de>,
		{
			Ok(NormalizedPathBuf::from(deserializer.deserialize_string(NormalizedPathBufVisitor)?))
		}
	}
}

#[derive(derive_more::Deref, derive_more::DerefMut, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub struct AsVec<T>(Vec<T>);
impl<T> From<Vec<T>> for AsVec<T> {
	fn from(x: Vec<T>) -> Self {
		AsVec(x)
	}
}
impl<T> From<T> for AsVec<T> {
	fn from(x: T) -> Self {
		AsVec(vec![x])
	}
}
impl<T: Clone> From<&[T]> for AsVec<T> {
	fn from(x: &[T]) -> Self {
		AsVec(x.to_vec())
	}
}
impl<T> Into<Vec<T>> for AsVec<T> {
	fn into(self) -> Vec<T> {
		self.0
	}
}

// cursed
pub fn dedup_unsorted<T: Hash + Eq>(vec: &mut Vec<T>) {
	struct PtrCmp<T: Hash + Eq> {
		ptr: *const T,
	}
	impl<T: Hash + Eq> Hash for PtrCmp<T> {
		fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
			unsafe { (*self.ptr).hash(state) };
		}
	}
	impl<T: Hash + Eq> PartialEq for PtrCmp<T> {
		fn eq(&self, other: &Self) -> bool {
			unsafe { *self.ptr == *other.ptr }
		}
	}
	impl<T: Hash + Eq> Eq for PtrCmp<T> {}

	if vec.len() == 2 {
		if vec[0] == vec[1] {
			vec.truncate(1);
		}
	} else if vec.len() > 2 {
		let mut dedup = HashSet::with_capacity(vec.len());
		let mut i = 0;
		while i != vec.len() {
			if !dedup.insert(PtrCmp { ptr: &vec[i] as *const T }) {
				vec.remove(i);
			} else {
				i += 1;
			}
		}
	}
}

pub fn stream_len<F: Seek>(f: &mut F) -> Result<u64, std::io::Error> {
	let old_pos = f.stream_position()?;
	let len = f.seek(SeekFrom::End(0))?;
	if old_pos != len {
		f.seek(SeekFrom::Start(old_pos))?;
	}

	Ok(len)
}

pub fn stream_bytes<R: Read, W: Write>(r: &mut BufReader<R>, w: &mut BufWriter<W>, mut bytes: usize) -> Result<(), std::io::Error> {
	Ok(loop {
		match r.fill_buf() {
			Ok([]) => break,
			Ok(data) if data.len() >= bytes => {
				w.write_all(&data[..bytes])?;
				break;
			}
			Ok(data) => {
				w.write_all(data)?;
				bytes -= data.len();
			}
			Err(e) if e.kind() == ErrorKind::Interrupted => {}
			Err(e) => return Err(e),
		}
	})
}

const BACKSLASH_BYTE: u8 = '\\' as u8;
const SINGLE_QUOTE_BYTE: u8 = '\'' as u8;
pub fn escape_single_quoted_json<S: Into<String>>(str: S) -> String {
	let mut str = str.into();
	let bytes: &mut Vec<u8> = unsafe { str.as_mut_vec() };
	let mut i = 0;
	while i < bytes.len() {
		let byte = bytes[i];
		match byte {
			BACKSLASH_BYTE | SINGLE_QUOTE_BYTE => {
				bytes.insert(i, BACKSLASH_BYTE);
				i += 1;
			}
			_ => {}
		}
		i += 1;
	}
	debug_assert!(String::from_utf8(bytes.to_vec()).is_ok());
	str
}

#[test]
fn test_escape_single_quoted_json() {
	let dangerous_json = String::from(
		r#"{"test":"don\\🚀🐱‍👤\\'t forget to escape me!🚀🐱‍👤","te🚀🐱‍👤st2":"don't forget to escape me!","test3":"\\🚀🐱‍👤\\\\'''\\\\🚀🐱‍👤\\\\🚀🐱‍👤\\'''''"}"#,
	);

	let definitely_escaped_dangerous_json = dangerous_json.clone().replace('\\', "\\\\").replace('\'', "\\'");
	let escape_single_quoted_json_test = escape_single_quoted_json(dangerous_json);

	let result = r#"{"test":"don\\\\🚀🐱‍👤\\\\\'t forget to escape me!🚀🐱‍👤","te🚀🐱‍👤st2":"don\'t forget to escape me!","test3":"\\\\🚀🐱‍👤\\\\\\\\\'\'\'\\\\\\\\🚀🐱‍👤\\\\\\\\🚀🐱‍👤\\\\\'\'\'\'\'"}"#;
	assert_eq!(definitely_escaped_dangerous_json, result);
	assert_eq!(escape_single_quoted_json_test, result);
}
