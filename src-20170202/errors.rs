use std::fmt;
use std::error::Error;
use config::MAX_NAME_SIZE;

// Name Errors
#[derive(Debug)]
pub enum NameError {
	Format(FormatError),
	Size(SizeError)
}
impl Error for NameError {
	fn description(&self) -> &str { 
		match *self {
			NameError::Format(ref err) => err.description(),
			NameError::Size(ref err) => err.description()
		}	 
	}
	fn cause(&self) -> Option<&Error> {  
		match *self {
			NameError::Format(_) => None,
			NameError::Size(_) => None
		}
	}
}
impl fmt::Display for NameError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			NameError::Format(ref err) => write!(f, "Name Format Error: {}", err),
			NameError::Size(ref err) => write!(f, "Name Size Error: {}", err)
		}
	}
}
#[derive(Debug)]
pub struct FormatError { msg: String }
impl FormatError {
	pub fn new(s: &str) -> FormatError { 
		FormatError { msg: format!("'{}' contains blanks.", s) }
	}
}
impl Error for FormatError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for FormatError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		write!(f, "{}", self.msg) 
	}
}
impl From<FormatError> for NameError {
	fn from(err: FormatError) -> NameError { NameError::Format(err) }
}
#[derive(Debug)]
pub struct SizeError { msg: String }
impl SizeError {
	pub fn new(n: usize) -> SizeError {
		SizeError { msg: format!("'{}' more than {} characters", n, MAX_NAME_SIZE) }
	}
}
impl Error for SizeError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for SizeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		write!(f, "'{}", self.msg) 
	}
}
impl From<SizeError> for NameError {
	fn from(err: SizeError) -> NameError { NameError::Size(err) }
}
// Tenant Errors
#[derive(Debug)]
pub enum TenantError {
	Name(NameError),
	DuplicateName(DuplicateNameError),
	Quota(QuotaError)
}
impl Error for TenantError {
	fn description(&self) -> &str {
		match *self {
			TenantError::DuplicateName(ref err) => err.description(),
			TenantError::Quota(ref err) => err.description(),
			TenantError::Name(ref err) => err.description()
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			TenantError::DuplicateName(_) => None,
			TenantError::Quota(_) => None,
			TenantError::Name(ref err) => Some(err)
		}
	}
}
impl fmt::Display for TenantError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			TenantError::DuplicateName(ref err) => write!(f, "Tenant Name Error: {}", err),
			TenantError::Quota(ref err) => write!(f, "Tenant Quota Error: {}", err),
			TenantError::Name(_) => write!(f, "Tenant Name Error caused by")
		}
	}
}
#[derive(Debug)]
pub struct QuotaError { msg: String }
impl QuotaError { 
	pub fn new(n: usize, available: usize) -> QuotaError {
		QuotaError { msg: format!("You asked for {} cells, but only {} are available", n, available) }
	}
}
impl Error for QuotaError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for QuotaError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<QuotaError> for TenantError {
	fn from(err: QuotaError) -> TenantError { TenantError::Quota(err) }
}
#[derive(Debug)]
pub struct DuplicateNameError { msg: String }
impl DuplicateNameError {
	pub fn new(id: &str) -> DuplicateNameError {
		DuplicateNameError { msg: format!("A tenant named '{}' already exists.", id) }
	}
}
impl Error for DuplicateNameError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for DuplicateNameError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<DuplicateNameError> for TenantError {
	fn from(err: DuplicateNameError) -> TenantError { TenantError::DuplicateName(err) }
}
impl From<NameError> for TenantError {
	fn from(err: NameError) -> TenantError { TenantError::Name(err) }
}

