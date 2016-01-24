use transport::Address;
use keyval::KeyVal;
use utils::{
  TransientOption,
  OneResult,
  unlock_one_result,
};
use std::io::{
  Write,
  Read,
  Result as IoResult,
};
use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};


/// A peer is a special keyval with an attached address over the network
pub trait Peer : KeyVal + 'static {
  type Address : Address;
  type Shadow : Shadow;
 // TODO rename to get_address or get_address_clone, here name is realy bad
  // + see if a ref returned would not be best (same limitation as get_key for composing types in
  // multi transport) + TODO alternative with ref to address for some cases
  fn to_address (&self) -> Self::Address;
  /// instantiate a new shadower for this peer (shadower wrap over write stream and got a lifetime
  /// of the connection). // TODO enum instead of bool!! (currently true for write mode false for
  /// read only)
  fn get_shadower (&self, write : bool) -> Self::Shadow;
//  fn to_address (&self) -> SocketAddr;

}


pub struct ShadowW<S : Shadow> (S);

pub struct ShadowR<S : Shadow> (S);

/// for shadowing capability
//Sync + Send + Clone + Debug + 'static {}
pub trait Shadow : Send + 'static {
  /// type of shadow to apply (most of the time this will be () or bool but some use case may
  /// require 
  /// multiple shadowing scheme, and therefore probably an enum type).
  type ShadowMode : Clone + Eq + Encodable + Decodable + Debug;
  /// get the header required for a shadow scheme : for instance the shadowmode representation and
  /// the salt. The action also make the shadow iter method to use the right state (internal state
  /// for shadow mode). TODO refactor to directly write in a Writer??
  fn shadow_header<W : Write> (&mut self, &mut W, &Self::ShadowMode) -> IoResult<()>;
  /// Shadow of message block (to use in the writer), and write it in writer. The function maps
  /// over write (see utils :: send_msg)
  fn shadow_iter<W : Write> (&mut self, &[u8], &mut W, &Self::ShadowMode) -> IoResult<usize>;
  /// flush at the end of message writing (useless to add content : reader could not read it),
  /// usefull to emptied some block cyper buffer, it does not flush the writer which should be
  /// flush manually (allow using multiple shadower in messages such as tunnel).
  fn shadow_flush<W : Write> (&mut self, w : &mut W, &Self::ShadowMode) -> IoResult<()>;
  /// read header getting mode and initializing internal information
  fn read_shadow_header<R : Read> (&mut self, &mut R) -> IoResult<Self::ShadowMode>;
  /// read shadow returning number of bytes read, probably using an internal buffer
  fn read_shadow_iter<R : Read> (&mut self, &mut R, buf: &mut [u8], &Self::ShadowMode) -> IoResult<usize>;
  /// all message but auth related one (PING  and PONG)
  fn default_message_mode () -> Self::ShadowMode;
  /// auth related messages (PING  and PONG)
  fn default_auth_mode () -> Self::ShadowMode;
  /// mode for tunnel headers or short messages
  fn default_head_mode () -> Self::ShadowMode;
 
  /// base for shadow iter using a symetric key (first param), mainly use in reply for tunnel
  /// message
  fn shadow_iter_sim<W : Write> (&mut self, &[u8], &[u8], &mut W, &Self::ShadowMode) -> IoResult<usize>;

  fn read_shadow_iter_sim<R : Read> (&mut self, &[u8], &mut R, buf : &mut [u8], &Self::ShadowMode) -> IoResult<usize>;
  fn shadow_sim_flush<W : Write> (&mut self, &mut W, &Self::ShadowMode) -> IoResult<()>;
  /// create a simkey TODO change to asref u8 in result, mainly use in reply for tunnel
  /// message
  fn shadow_simkey(&mut self, &Self::ShadowMode) -> Vec<u8>;
 
}

/// a struct for reading once with shadow
pub struct ShadowReadOnce<'a, S : 'a + Shadow, R : 'a + Read>(&'a mut S,Option<<S as Shadow>::ShadowMode>, &'a mut R);

impl<'a, S : 'a + Shadow, R : 'a + Read> ShadowReadOnce<'a,S,R> {
  pub fn new(s : &'a mut S, r : &'a mut R) -> Self {
    ShadowReadOnce(s, None, r)
  }

}

impl<'a, S : 'a + Shadow, R : 'a + Read> Read for ShadowReadOnce<'a,S,R> {
  fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
    if self.1.is_none() {
      let sm = try!(self.0.read_shadow_header(self.2));
      self.1 = Some(sm);
    }
    self.0.read_shadow_iter(&mut self.2, buf, self.1.as_ref().unwrap())
  }
}

/// a struct for writing once with shadow (in a write abstraction)
/// Flush does not lead to a new header write (for this please create a new shadowWriteOnce).
/// Flush does not flush the inner writer but only the shadower (to flush writer please first end 
pub struct ShadowWriteOnce<'a, S : 'a + Shadow, W : 'a + Write>(&'a mut S, &'a <S as Shadow>::ShadowMode, &'a mut W, bool);




/// layerable shadowrite once (fat pointer on reader and recursive flush up to terminal reader). //
/// last bool is to tell if it is first (reader is not flush on first)
pub struct ShadowWriteOnceL<'a, S : Shadow>(S, <S as Shadow>::ShadowMode, &'a mut Write, bool,bool);



/// Multiple layered shadow write
pub struct MShadowWriteOnce<'a, S : 'a + Shadow, W : 'a + Write>(&'a mut [S], &'a <S as Shadow>::ShadowMode, &'a mut W, &'a mut [bool]);


impl<'a, S : 'a + Shadow, W : 'a + Write> ShadowWriteOnce<'a,S,W> {
  pub fn new(s : &'a mut S, r : &'a mut W, sm : &'a <S as Shadow>::ShadowMode) -> Self {
    ShadowWriteOnce(s, sm, r, false)
  }
}

// TODO delete it
impl<'a, S : 'a + Shadow> ShadowWriteOnceL<'a,S> {
  pub fn new(s : S, r : &'a mut Write, sm : <S as Shadow>::ShadowMode, is_first : bool) -> Self {
    ShadowWriteOnceL(s, sm, r, false, is_first)
  }
}
impl<'a, S : 'a + Shadow, W : 'a + Write> MShadowWriteOnce<'a,S,W> {
  pub fn new(s : &'a mut [S], r : &'a mut W, sm : &'a <S as Shadow>::ShadowMode, writtenhead : &'a mut [bool]) -> Self {
    MShadowWriteOnce(s, sm, r, writtenhead)
  }
}

impl<'a, S : 'a + Shadow, W : 'a + Write> Write for MShadowWriteOnce<'a,S,W> {
  fn write(&mut self, cont: &[u8]) -> IoResult<usize> {
    if !(self.3)[0] {
      // write header
      if self.0.len() > 1 {
      if let Some((f,last)) = self.0.split_first_mut() {
      let mut el = MShadowWriteOnce(last, self.1, self.2, &mut self.3[1..]); 
      try!(f.shadow_header(&mut el, self.1));
      }} else {
        try!((self.0).get_mut(0).unwrap().shadow_header(self.2, self.1));
      }
      (self.3)[0] = true;
    }
    if self.0.len() > 1 {
    if let Some((f,last)) = self.0.split_first_mut() {
      let mut el = MShadowWriteOnce(last, self.1, self.2, &mut self.3[1..]); 
      return f.shadow_iter(cont, &mut el, self.1);
    }
    }
    // last
    (self.0).get_mut(0).unwrap().shadow_iter(cont, self.2, self.1)
  }
  fn flush(&mut self) -> IoResult<()> {
    if self.0.len() > 1 {
    if let Some((f,last)) = self.0.split_first_mut()  {
      let mut el = MShadowWriteOnce(last, self.1, self.2, &mut self.3[1..]); 
      try!(f.shadow_flush(&mut el, self.1));
      return el.flush();
    }
    }
    // last do not flush writer
    (self.0).get_mut(0).unwrap().shadow_flush(self.2, self.1)
 
  }
}
/*
impl<'a, S : 'a + Shadow> Write for MShadowWriteOnce<'a,S> {

  fn write(&mut self, cont: &[u8]) -> IoResult<usize> {
    for i in cont.len() .. 0 {
      if !self.3[i-1] {
        try!(self.0.shadow_header(&mut self.2, &self.1));
        self.3 = true;
      }
      self.0.get_mut(i-1).unwrap().shadow_iter(cont, &mut self.2, &self.1)
    }
  }
 
  /// flush does not flush the first inner writer
  fn flush(&mut self) -> IoResult<()> {
    // do not flush last
    for i in 0 .. cont.len() - 1 {
      try!(self.0.get_mut(i).unwrap().shadow_flush(&mut self.2, &self.1));
    }
  }
}*/




impl<'a, S : 'a + Shadow, W : 'a + Write> Write for ShadowWriteOnce<'a,S,W> {

  fn write(&mut self, cont: &[u8]) -> IoResult<usize> {
    if !self.3 {
      try!(self.0.shadow_header(&mut self.2, &self.1));
      self.3 = true;
    }
    self.0.shadow_iter(cont, &mut self.2, &self.1)
  }
 
  /// flush does not flush the inner writer
  fn flush(&mut self) -> IoResult<()> {
    self.0.shadow_flush(&mut self.2, &self.1)
  }
}

impl<'a, S : 'a + Shadow> Write for ShadowWriteOnceL<'a,S> {

  fn write(&mut self, cont: &[u8]) -> IoResult<usize> {
    if !self.3 {
      try!(self.0.shadow_header(&mut self.2, &self.1));
      self.3 = true;
    }
    self.0.shadow_iter(cont, &mut self.2, &self.1)
  }
 
  /// flush does not flush the first inner writer
  fn flush(&mut self) -> IoResult<()> {
    try!(self.0.shadow_flush(&mut self.2, &self.1));
    if !self.4 {
      self.2.flush()
    } else {
      Ok(())
    }
  }
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
    Ok(())
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
  #[inline]
  fn default_head_mode () -> Self::ShadowMode {
    ()
  }
 
  #[inline]
  fn shadow_iter_sim<W : Write> (&mut self, _ : &[u8], m : &[u8], w : &mut W, _ : &Self::ShadowMode) -> IoResult<usize> {
    w.write(m)
  }
  #[inline]
  fn read_shadow_iter_sim<R : Read> (&mut self, _ : &[u8], r : &mut R, buf : &mut [u8], _ : &Self::ShadowMode) -> IoResult<usize> {
    r.read(buf)
  }
 
  #[inline]
  fn shadow_sim_flush<W : Write> (&mut self, w : &mut W, _ : &Self::ShadowMode) -> IoResult<()> {
    w.flush()
  }
  fn shadow_simkey(&mut self, _ : &Self::ShadowMode) -> Vec<u8> {
    Vec::new()
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



#[derive(RustcDecodable,RustcEncodable,Debug,PartialEq,Clone)]
/// State of a peer
pub enum PeerState {
  /// accept running on peer return None, it may be nice to store on heavy accept but it is not
  /// mandatory
  Refused,
  /// some invalid frame and others lead to block a peer, connection may be retry if address
  /// changed for the peer
  Blocked(PeerPriority),
  /// This priority is more an internal state and should not be return by accept.
  /// peers has not yet been ping or connection broke
  Offline(PeerPriority),
 /// This priority is more an internal state and should not be return by accept.
  /// sending ping with given challenge string, a PeerPriority is set if accept has been run
  /// (lightweight accept) or Unchecked
  Ping(Vec<u8>,TransientOption<OneResult<bool>>,PeerPriority),
  /// Online node
  Online(PeerPriority),
}

/// Ping ret one result return false as soon as we do not follow it anymore
impl Drop for PeerState {
    fn drop(&mut self) {
        debug!("Drop of PeerState");
        match self {
          &mut PeerState::Ping(_,ref mut or,_) => {
            or.0.as_ref().map(|r|unlock_one_result(&r,false)).is_some();
            ()
          },
          _ => (),
        }
    }
}


impl PeerState {
  pub fn get_priority(&self) -> PeerPriority {
    match self {
      &PeerState::Refused => PeerPriority::Unchecked,
      &PeerState::Blocked(ref p) => p.clone(),
      &PeerState::Offline(ref p) => p.clone(),
      &PeerState::Ping(_,_,ref p) => p.clone(),
      &PeerState::Online(ref p) => p.clone(),
    }
  }
  pub fn new_state(&self, change : PeerStateChange) -> PeerState {
    let pri = self.get_priority();
    match change {
      PeerStateChange::Refused => PeerState::Refused,
      PeerStateChange::Blocked => PeerState::Blocked(pri),
      PeerStateChange::Offline => PeerState::Offline(pri),
      PeerStateChange::Online  => PeerState::Online(pri), 
      PeerStateChange::Ping(chal,or)  => PeerState::Ping(chal,or,pri), 
    }
  }
}


#[derive(Debug,PartialEq,Clone)]
/// Change to State of a peer
pub enum PeerStateChange {
  Refused,
  Blocked,
  Offline,
  Online,
  Ping(Vec<u8>, TransientOption<OneResult<bool>>),
}


