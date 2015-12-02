

#[macro_use] extern crate log;
extern crate rustc_serialize;
extern crate time;
extern crate num;
extern crate rand;
extern crate bit_vec;
extern crate byteorder;


#[macro_export]
/// a try which use a wrapping type
macro_rules! tryfor(($ty:ident, $expr:expr) => (

  try!(($expr).map_err(|e|$ty(e)))

));
 
#[macro_export]
/// Automatic define for KeyVal without attachment
macro_rules! noattachment(() => (
  fn get_attachment(&self) -> Option<&Attachment>{
    None
  }
));

#[macro_export]
/// convenience macro for peer implementation without a shadow
macro_rules! noshadow(() => (

  type Shadow = NoShadow;
  #[inline]
  fn get_shadower (&self, _ : bool) -> Self::Shadow {
    NoShadow
  }
));

pub mod utils;
pub mod keyval;
pub mod kvstore;
pub mod simplecache;
pub mod peer;
pub mod kvcache;
pub mod transport;
pub mod mydhtresult;
pub mod msgenc;
pub mod query;
pub mod procs;
pub mod rules;
//pub mod route;

