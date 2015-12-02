use std::io::Write;
use std::io::Read;
use std::io::Result as IoResult;
use transport::Address;
use keyval::KeyVal;


/// A peer is a special keyval with an attached address over the network
pub trait Peer : KeyVal + 'static {
  type Address : Address;
  type Shadow : Shadow;
 // TODO rename to get_address or get_address_clone, here name is realy bad
  // + see if a ref returned would not be best (same limitation as get_key for composing types in
  // multi transport)
  fn to_address (&self) -> Self::Address;
  /// instantiate a new shadower for this peer (shadower wrap over write stream and got a lifetime
  /// of the connection). // TODO enum instead of bool!!
  fn get_shadower (&self, write : bool) -> Self::Shadow;
//  fn to_address (&self) -> SocketAddr;
}

/// for shadowing capability
//Sync + Send + Clone + Debug + 'static {}
pub trait Shadow : Send + 'static {
  /// type of shadow to apply (most of the time this will be () or bool but some use case may
  /// require 
  /// multiple shadowing scheme, and therefore probably an enum type).
  type ShadowMode;
  /// get the header required for a shadow scheme : for instance the shadowmode representation and
  /// the salt. The action also make the shadow iter method to use the right state (internal state
  /// for shadow mode). TODO refactor to directly write in a Writer??
  fn shadow_header<W : Write> (&mut self, &mut W, &Self::ShadowMode) -> IoResult<()>;
  /// Shadow of message block (to use in the writer), and write it in writer. The function maps
  /// over write (see utils :: send_msg)
  fn shadow_iter<W : Write> (&mut self, &[u8], &mut W, &Self::ShadowMode) -> IoResult<usize>;
  /// flush at the end of message writing (useless to add content : reader could not read it),
  /// usefull to emptied some block cyper buffer.
  fn shadow_flush<W : Write> (&mut self, w : &mut W, &Self::ShadowMode) -> IoResult<()>;
  /// read header getting mode and initializing internal information
  fn read_shadow_header<R : Read> (&mut self, &mut R) -> IoResult<Self::ShadowMode>;
  /// read shadow returning number of bytes read, probably using an internal buffer
  fn read_shadow_iter<R : Read> (&mut self, &mut R, buf: &mut [u8], &Self::ShadowMode) -> IoResult<usize>;
  /// all message but auth related one (PING  and PONG)
  fn default_message_mode () -> Self::ShadowMode;
  /// auth related messages (PING  and PONG)
  fn default_auth_mode () -> Self::ShadowMode;
}

pub struct NoShadow;

impl Shadow for NoShadow {
  type ShadowMode = ();
  #[inline]
  fn shadow_header<W : Write> (&mut self, _ : &mut W, _ : &Self::ShadowMode) -> IoResult<()> {
    Ok(()) 
  }
  #[inline]
  fn shadow_iter<W : Write> (&mut self, m : &[u8], w : &mut W, _ : &Self::ShadowMode) -> IoResult<usize> {
    w.write(m)
  }
  #[inline]
  fn shadow_flush<W : Write> (&mut self, w : &mut W, _ : &Self::ShadowMode) -> IoResult<()> {
    w.flush()
  }
  #[inline]
  fn read_shadow_header<R : Read> (&mut self, _ : &mut R) -> IoResult<Self::ShadowMode> {
    Ok(())
  }
  #[inline]
  fn read_shadow_iter<R : Read> (&mut self, r : &mut R, buf: &mut [u8], _ : &Self::ShadowMode) -> IoResult<usize> {
    r.read(buf)
  }
 
  #[inline]
  fn default_message_mode () -> Self::ShadowMode {
    ()
  }
  #[inline]
  fn default_auth_mode () -> Self::ShadowMode {
    ()
  }
}


#[derive(RustcDecodable,RustcEncodable,Debug,PartialEq,Clone)]
/// State of a peer
pub enum PeerPriority {
  /// peers is online but not already accepted
  /// it is used when receiving a ping, peermanager on unchecked should ping only if auth is
  /// required
  Unchecked,
  /// online, no prio distinction between peers
  Normal,
  /// online, with a u8 priority
  Priority(u8),
}

