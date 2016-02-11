//! tunnel : primitive function for tunnel (for both modes
//!
//! TODO reader and writer with possible two different size limiter for het mode (head limiter
//! should be small when content big or growable)
//!

use rand::thread_rng;
use rand::Rng;
use bincode::SizeLimit;
use transport::Address;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use bincode::rustc_serialize::{
  encode_into as bin_encode, 
  decode_from as bin_decode,
};
use bincode::rustc_serialize::{
  encode,
  decode,
};
use std::convert::Into;
use keyval::KeyVal;
use peer::Peer;
use peer::{
  Shadow,
  ShadowReadOnce,
  ShadowWriteOnce,
  ShadowWriteOnceL,
  new_shadow_read_once,
  new_shadow_write_once,
};
use std::io::{
  Write,
  Read,
  Result,
  Error as IoError,
  ErrorKind as IoErrorKind,
};
use readwrite_comp::{
  MultiW,
  MultiWExt,
  new_multiw,
  ExtRead,
  ExtWrite,
  CompW,
  CompWState,
  CompR,
  CompRState,
  CompExtW,
  CompExtWInner,
  CompExtR,
  CompExtRInner,
};

use mydhtresult::Error;

//use std::marker::Reflect;
use bincode::rustc_serialize::EncodingError as BincError;
use bincode::rustc_serialize::DecodingError as BindError;

pub struct BincErr(BincError);
impl From<BincErr> for IoError {
  #[inline]
  fn from(e : BincErr) -> IoError {
    IoError::new(IoErrorKind::Other, e.0)
  }
}
pub struct BindErr(BindError);
impl From<BindErr> for IoError {
  #[inline]
  fn from(e : BindErr) -> IoError {
    IoError::new(IoErrorKind::Other, e.0)
  }
}


/// to resend message in another tunnel when error
pub type TunnelID = usize;


#[derive(RustcDecodable,RustcEncodable,Debug,Clone,PartialEq,Eq)]
/// Query Mode defines the way our DHT communicate, it is very important.
/// Unless messageencoding does not serialize it, all query mode could be used. 
/// For some application it could be relevant to forbid the reply to some query mode : 
/// TODO implement a filter in server process (and client proxy).
/// When a nb hop is in tunnel mode it could be changed or not be the same as the actual size : it
/// is only an indication of the size to use to proxy query (or to reply for publicTunnel).
pub enum TunnelMode {
  NoTunnel, // TODO remove as in this case we use directly writer
  /// Tunnel using a single path (forward path = backward path). Parameter is number of hop in the
  /// tunnel (1 is direct).
  /// Error take tunnel backward (error is unencrypted (we do not know originator) and therefore
  /// not explicit : id node could not proxied.
  /// For full tunnel shadow mode it is encrypted with every keys (and temp key for every hop are
  /// transmitted to dest for reply (every hop got it packed to)).
  Tunnel(u8,TunnelShadowMode),
  /// Tunnel with different path forward and backward.
  /// param is nb of hop.
  /// Error will use forward route like normal tunnel if boolean is true, or backward route (very
  /// likely to fail and result in query timeout) if boolean is false
  BiTunnel(u8,TunnelShadowMode,bool),
 
  /// in this mode origin and destination know themselve (cf message mode), we simply reply with same nb hop. Back
  /// route is therefore from dest (no need to include back route info.
  PublicTunnel(u8,TunnelShadowMode),

}

#[derive(RustcDecodable,RustcEncodable,Debug,Clone,PartialEq,Eq)]
pub enum TunnelShadowMode {
  /// NoShadow for testing shadow is not applied
  NoShadow,
  /// shadow all with full layer, in default mode 
  Full,
  /// Dest key only encryption for content in default mode, header is layered fith header in
  /// default mode, header is layered with header mode
  Last,
  /// Same as last but header is using same shadower (default as content) TODO for now not
  /// implemented
  LastDefault,
}

impl TunnelMode {
  pub fn tunnel_shadow_mode(&self) -> TunnelShadowMode {
     match self {
      &TunnelMode::NoTunnel => TunnelShadowMode::NoShadow,
      &TunnelMode::Tunnel(_,ref b) 
      | &TunnelMode::BiTunnel(_,ref b,_) 
      | &TunnelMode::PublicTunnel(_,ref b) => b.clone(),
    }
  }
  pub fn is_full_enc(&self) -> bool {
    TunnelShadowMode::Full == self.tunnel_shadow_mode()
  }
  /// do we apply head mode TODO replace by is het everywhere
  pub fn head_mode(&self) -> bool {
    TunnelShadowMode::Last == self.tunnel_shadow_mode()
  }

  // do we use distinct encoding for content
  pub fn is_het(&self) -> bool {
    self.tunnel_shadow_mode().is_het()
  }

}
impl TunnelShadowMode {
  // do we use distinct encoding for content
  pub fn is_het(&self) -> bool {
    &TunnelShadowMode::Last == self
  }
}
#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
/// QueryMode info to use in message between peers.
/// TODO delete : it is info that are read/write directly in methods (tunnelid)
pub enum TunnelModeMsg {
  Tunnel(u8,Option<usize>, TunnelID, TunnelShadowMode), //first u8 is size of tunnel for next hop of query, usize is size of descriptor for proxying info, if no usize it means it is fully shadowed and a pair is under it
  BiTunnel(u8,bool,Option<usize>,TunnelID,TunnelShadowMode), // see tunnel, plus if bool is true we error reply with forward route stored as laste param TODO replace Vec<u8> by Reader (need encodable of this reader as writalll
  PublicTunnel(u8,usize,TunnelID,TunnelShadowMode),
}

#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
pub struct TunnelProxyInfo<P : Peer> {
  pub next_proxy_peer : Option<<P as Peer>::Address>,
  pub tunnel_id : usize, // tunnelId change for every hop
  pub tunnel_id_failure : Option<usize>, // if set that is a failure
}
#[derive(RustcEncodable,Debug,Clone)]
pub struct TunnelProxyInfoSend<'a, P : Peer> {
  pub next_proxy_peer : Option<&'a <P as Peer>::Address>,
  pub tunnel_id : usize, // tunnelId change for every hop
  pub tunnel_id_failure : Option<usize>, // if set that is a failure
}


/*
impl<P : Peer> TunnelProxyInfo<P> {
  fn write_as_bytes<W:Write> (&self, w : &mut W) -> Result<()> {
    match self.next_proxy_peer {
      Some(ref add) => {
        try!(w.write(&[1]));
        try!(bin_encode(add, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
      },
      None => {
        try!(w.write(&[0]));
      },
    };
    try!(w.write_u64::<LittleEndian>(self.tunnel_id as u64));
    match self.tunnel_id_failure {
      Some(idf) => {
        try!(w.write(&[1]));
        try!(w.write_u64::<LittleEndian>(idf as u64));
      },
      None => {
        try!(w.write(&[0]));
      },
    };
    Ok(())
  }

  fn read_as_bytes<R:Read> (r : &mut R) -> Result<Self> {
    let mut buf = [0];
    let obuff = &mut buf[..];
    try!(r.read(obuff));
    let npp = if obuff[0] == 1 {
//      let add = try!(<P as Peer>::Address::read_as_bytes(r));
      let add = try!(bin_decode(r, SizeLimit::Infinite).map_err(|e|BindErr(e)));
      Some(add)
    } else {
      None
    };
    let tuid = try!(r.read_u64::<LittleEndian>());
    try!(r.read(obuff));
    let tif = if obuff[0] == 1 {
      let fid = try!(r.read_u64::<LittleEndian>());
      Some(fid as usize)
    } else {
      None
    };
    Ok(TunnelProxyInfo {
      next_proxy_peer : npp,
      tunnel_id : tuid as usize,
      tunnel_id_failure : tif,
    })
  }

}*/

impl TunnelModeMsg {
  /// get corresponding querymode
  pub fn get_mode (&self) -> TunnelMode {
    match self {

      &TunnelModeMsg::Tunnel (l,_,_,ref s1) => TunnelMode::Tunnel(l,s1.clone()),
      &TunnelModeMsg::BiTunnel (l,f,_,_,ref s1) => TunnelMode::BiTunnel(l,s1.clone(),f),
      &TunnelModeMsg::PublicTunnel(l,_,_,ref s1) => TunnelMode::PublicTunnel(l,s1.clone()),
    }
   
  }
  pub fn get_tunnelid (&self) -> &TunnelID {
    match self {
      &TunnelModeMsg::Tunnel (_,_,ref tid,_) => tid,
      &TunnelModeMsg::BiTunnel (_,_,_,ref tid,_) => tid,
      &TunnelModeMsg::PublicTunnel(_,_,ref tid, _) => tid,
    }
  }
  pub fn get_tunnel_length (&self) -> u8 {
    match self {
      &TunnelModeMsg::Tunnel (l,_,_,_) => l,
      &TunnelModeMsg::BiTunnel (l,_,_,_,_) => l,
      &TunnelModeMsg::PublicTunnel(l,_,_, _) => l,
    }
  }
  pub fn get_shadow_mode (&self) -> TunnelShadowMode {
    match self {
      &TunnelModeMsg::Tunnel (_,_,_,ref s) => s.clone(),
      &TunnelModeMsg::BiTunnel (_,_,_,_,ref s) => s.clone(),
      &TunnelModeMsg::PublicTunnel(_,_,_,ref s) => s.clone(),
    }
  }
}

pub type TunnelReader<'a, 'b, E : ExtRead + 'b, P : Peer + 'b, R : 'a + Read> = CompR<'a,'b,R,TunnelReaderExt<E,P>>;
/// a reader ext for a proxy payload. This is not using multiR (only one layer per peer, but we use a similar to TunnelShadow internal reader to allow it).
/// TunnelWriter is simply a compw over R.
/// The reader is not optimal : it use two read shadower one for content and one for header (it
/// could only use one in most case but we do not know it beforer reading frame header).
/// E is the frame limiter used (see bytes_wr).
pub struct TunnelReaderExt<E : ExtRead, P : Peer> {
  shadow: TunnelShadowR<E,P>, // our decoding
  shacont: <P as Peer>::Shadow, // our decoding
  shanocont: E, // for proxying
  mode: TunnelMode,
}
impl<E : ExtRead, P : Peer> TunnelReaderExt<E, P> {
  #[inline]
  pub fn is_dest(&self) -> Option<bool> {
    self.shadow.1.as_ref().map(|tpi|tpi.next_proxy_peer.is_none())
  }
  #[inline]
  pub fn as_reader<'a,'b,R : Read>(&'b mut self, r : &'a mut R) -> TunnelReader<'a, 'b, E, P, R> {
    CompR::new(r,self)
  }
} 
impl<E : ExtRead + Clone, P : Peer> TunnelReaderExt<E, P> {
  // new, if tunnel mode is already read or static it could be set but reader (from as_reader) could not be used
  // afterward (it would read header again)).
  pub fn new(p : &P, e : E, mode : Option<TunnelMode>) -> TunnelReaderExt<E, P> {
    let mut s1 = p.get_shadower(false);
    let mut s2 = p.get_shadower(false);
    let m = mode.unwrap_or(TunnelMode::NoTunnel); // default to no tunnel

    TunnelReaderExt {
      shadow : TunnelShadowR(CompExtR(e.clone(),s1),None),
      shacont : s2,
      shanocont : e,
      mode : m,
    }
  }
}

impl<E : ExtRead, P : Peer> ExtRead for TunnelReaderExt<E, P> {
  #[inline]
  fn read_header<R : Read>(&mut self, r : &mut R) -> Result<()> {
    let tun_mode = try!(bin_decode(r, SizeLimit::Infinite).map_err(|e|BindErr(e)));
    println!("tun mode : {:?}",tun_mode);
    match tun_mode {
       TunnelMode::NoTunnel => self.shadow.1 = Some(TunnelProxyInfo {
          next_proxy_peer : None,
          tunnel_id : 0,
          tunnel_id_failure : None,
       }), // to be dest

       _ => {
        try!(self.shadow.read_header(r));
        if tun_mode.is_het() {
          if let Some(true) = self.is_dest() {
            // try to read shacont header
{            let mut inw  = CompExtRInner(r, &mut self.shadow);
            try!(self.shacont.read_header(&mut inw)); }
            try!(self.shadow.read_end(r));
            try!(self.shanocont.read_header(r)); // header of stop for last only
          }
        };
      }
    };
    self.mode = tun_mode;
    Ok(())
  }
 
  #[inline]
  fn read_from<R : Read>(&mut self, r : &mut R, buf: &mut [u8]) -> Result<usize> {
    if let TunnelMode::NoTunnel = self.mode {
      r.read(buf)
    } else {
    if self.mode.is_het() {
      if let Some(true) = self.is_dest() {
        let mut inw  = CompExtRInner(r, &mut self.shanocont);
        self.shacont.read_from(&mut inw,buf)
      } else {
        // just read header (specific code for content in tunnel proxy function
        self.shadow.read_from(r,buf)
      }
    } else {
      self.shadow.read_from(r,buf)
    }
    }
  }
  #[inline]
  fn read_end<R : Read>(&mut self, r : &mut R) -> Result<()> {
    if let TunnelMode::NoTunnel = self.mode {
    } else {
    if self.mode.is_het() {
      if let Some(true) = self.is_dest() {
        {let mut inw  = CompExtRInner(r, &mut self.shanocont);
        self.shacont.read_end(&mut inw);
        }
        self.shanocont.read_end(r);
      } else {
        self.shadow.read_end(r);
       // self.shanocont.read_end(r);
      }
    } else {
      self.shadow.read_end(r);
    }
    }
    Ok(())
  }
}


/// a reader for a proxy payload
pub struct TunnelReaderExt2<'a, P : Peer, R : 'a + Read> {
  // payload remaining size
  rem_size : usize,
  buf: &'a mut [u8],
  mode: Option<TunnelMode>, // mode is read from first frame TODO remove as it is done by CompR state
  shadow: <P as Peer>::Shadow, // our decoding
  shacont: <P as Peer>::Shadow, // our decoding for corntent if het
  reader: &'a mut R,
  proxyinfo : Option<TunnelProxyInfo<P>>, // is initialized after first read
}

/// writer to encode for tunneling, it create the header and encode the content
/// Note that all peers are of the same kind (same for shadow) as we currently support only one
/// Peer in one dht (no fat pointer here)
pub struct TunnelWriterExt2<'a, P : Peer, W : 'a + Write> {
  buf: &'a mut [u8],
  bufix: usize,
  addrs: Vec<<P as Peer>::Address>, // of none it means that heading hasbeen sent otherwise it need to be sent
  shads: Vec<<P as Peer>::Shadow>, // of none it means that heading hasbeen sent otherwise it need to be sent
  shacont: Option<<P as Peer>::Shadow>, // shadow for content when heterogenous enc
  hopids: Vec<usize>, // Id of all hop to report error
  error: Option<usize>, // possible error hop to report
  hasheader: bool,
  mode: TunnelMode,
  sender: &'a mut W,
}


pub type TunnelWriter<'a, 'b, E : ExtWrite + 'b, P : Peer + 'b, W : 'a + Write> = CompW<'a,'b,W,TunnelWriterExt<E,P>>;
pub struct TunnelWriterExt<E : ExtWrite, P : Peer> {
  shads: MultiWExt<TunnelShadowW<E,P>>,
  shacont: Option<CompExtW<<P as Peer>::Shadow,E>>, // shadow for content when heterogenous enc and last : the reader need to know the size of its content but E is external
  error: Option<usize>, // possible error hop to report
  mode: TunnelMode,
}
impl<E : ExtWrite + Clone, P : Peer> TunnelWriterExt<E, P> {
  #[inline]
  pub fn as_writer<'a,'b,W : Write>(&'b mut self, w : &'a mut W) -> TunnelWriter<'a, 'b, E, P, W> {
    CompW::new(w,self)
  }
  // new
  pub fn new(peers : &[&P], e : E, mode : TunnelMode, error : Option<usize>, 
    headmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
    contmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
  ) -> TunnelWriterExt<E, P> {
  
    let nbpeer = peers.len();
    let mut shad = Vec::with_capacity(nbpeer - 1);

    if let TunnelMode::NoTunnel = mode {
      // no shadow
    } else {
      let mut thrng = thread_rng();
      let mut geniter = thrng.gen_iter();
      let mut next_proxy_peer = None;

println!("in {}", peers.len());
      for i in  (1 .. peers.len()).rev() { // do not add first (is origin)
println!("in + {}", i);
        let p = peers.get(i).unwrap();
println!("inbis");
        let tpi = TunnelProxyInfo {
          next_proxy_peer : next_proxy_peer,
          tunnel_id : geniter.next().unwrap(),
          tunnel_id_failure : None, // if set that is a failure
        };
        next_proxy_peer = Some(p.to_address());
        let mut s = p.get_shadower(true);
        if mode.is_het() {
          s.set_mode(headmode.clone());
        } else {
          s.set_mode(contmode.clone());
        }
        shad.push(TunnelShadowW(CompExtW(e.clone(),s), tpi));
println!("push");
      }
    }
    let shacont = if mode.is_het() {
      peers.last().map(|p|{
        let mut s = p.get_shadower(true);
        s.set_mode(contmode.clone());
        CompExtW(s,e.clone())
      })
    } else {
      None
    };
    TunnelWriterExt {
      shads : MultiWExt::new(shad),
      shacont : shacont,
      error: error,
      mode: mode,
    }
  }
}

impl<E : ExtWrite, P : Peer> ExtWrite for TunnelWriterExt<E, P> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(bin_encode(&self.mode, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    if let TunnelMode::NoTunnel = self.mode {
      Ok(())
    } else {
    try!(self.shads.write_header(w));
    match self.shacont.as_mut() {
      Some (s) => {
        { // cont enc header
          let mut inw  = CompExtWInner(w, &mut self.shads);
          try!(s.0.write_header(&mut inw)); // enc header readable only for dest
        }
        try!(self.shads.write_end(w));
        try!(self.shads.flush_into(w));
        try!(s.1.write_header(w)); // end header readable for proxy
      }
      None => (),
    }
    Ok(())
    }
  }
  #[inline]
  fn write_into<W : Write>(&mut self, w : &mut W, cont: &[u8]) -> Result<usize> {
    if let TunnelMode::NoTunnel = self.mode {
      w.write(cont)
    } else {
    match self.shacont.as_mut() {
      Some (s) => s.write_into(w,cont),
      None => self.shads.write_into(w,cont),
    }
    }
  }
  #[inline]
  fn flush_into<W : Write>(&mut self, w : &mut W) -> Result<()> {
    if let TunnelMode::NoTunnel = self.mode {
      Ok(())
    } else {
    match self.shacont.as_mut() {
      Some (s) => s.flush_into(w),
      None => self.shads.flush_into(w),
    }
    }
  }
  #[inline]
  fn write_end<W : Write>(&mut self, w : &mut W) -> Result<()> {
    if let TunnelMode::NoTunnel = self.mode {
      Ok(())
    } else {
    match self.shacont.as_mut() {
      Some (s) => s.write_end(w),
      None => self.shads.write_end(w),
    }
    }
  }
}

/// override shadow for tunnel with custom ExtRead and ExtWrite over Shadow
/// First ExtWrite is bytes_wr to use for proxying content (get end of encoded stream).
pub struct TunnelShadowW<E : ExtWrite, P : Peer> (pub CompExtW<E,<P as Peer>::Shadow>, pub TunnelProxyInfo<P>);

pub struct TunnelShadowR<E : ExtRead, P : Peer> (pub CompExtR<E,<P as Peer>::Shadow>, pub Option<TunnelProxyInfo<P>>);

impl<E : ExtWrite, P : Peer> ExtWrite for TunnelShadowW<E,P> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(self.0.write_header(w));
    let mut inw  = CompExtWInner(w, &mut self.0);
    try!(bin_encode(&self.1, &mut inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    Ok(())
  }
  #[inline]
  fn write_into<W : Write>(&mut self, w : &mut W, cont : &[u8]) -> Result<usize> {
    self.0.write_into(w,cont)
  }   
  #[inline]
  fn flush_into<W : Write>(&mut self, w : &mut W) -> Result<()> {
    self.0.flush_into(w)
  }
  #[inline]
  fn write_end<W : Write>(&mut self, w : &mut W) -> Result<()> {
    self.0.write_end(w)
  }
}

impl<E : ExtRead, P : Peer> ExtRead for TunnelShadowR<E,P> {
  #[inline]
  fn read_header<R : Read>(&mut self, r : &mut R) -> Result<()> {
    try!(self.0.read_header(r));
    let mut inr  = CompExtRInner(r, &mut self.0);
    let tpi : TunnelProxyInfo<P> = try!(bin_decode(&mut inr, SizeLimit::Infinite).map_err(|e|BindErr(e)));
    self.1 = Some(tpi);
    Ok(())
  }
  #[inline]
  fn read_from<R : Read>(&mut self, r : &mut R, buf: &mut [u8]) -> Result<usize> {
    self.0.read_from(r,buf)
  }
  #[inline]
  fn read_end<R : Read>(&mut self, r : &mut R) -> Result<()> {
    self.0.read_end(r)
  }
}

 
/// iterate on it to empty reader into sender  and flush with additional content (shadow is same kind for reader (ourself) and dest as a tunnel is
/// using only on kind).
///(do not write header and behave according to mode)
/// Primitive to proxy (could be simple function).
/// TunnelReaderExt must have already read its header from R (otherwhise it is unknow if we can
/// proxy or not).
/// TODO er and ew are used only for het, maybe split function in two.
pub fn proxy_content<
  P : Peer,
  ER : ExtRead,
  R : Read,
  EW : ExtWrite,
  W : Write> ( 
  buf : &mut [u8], 
  tre : &mut TunnelReaderExt<ER,P>,
  mut er : ER,
  mut ew : EW,
  r : &mut R,
  w : &mut W) -> Result<()> {
    {
//    try!(tw.write_header(w));
    let mut sr = 1;
    while sr != 0 {
      sr = try!(tre.read_from(r, buf));
      let mut sw = sr;
      while sw > 0 {
        sw = try!(w.write(&buf[..sr]));
        if sw == 0 {
          return Err(IoError::new(IoErrorKind::Other, "Proxying failed, it seems we do not write all content"));
        }
      }
    }
    try!(tre.read_end(r));
    }
    // for het we only proxied the layered header : need to proxy the payload (end write of one
    // terminal shadow) : we read with end handling and write with same end handling
    if tre.mode.is_het() {
      try!(er.read_header(r));
      try!(ew.write_header(w));
      let mut sr = 1;
      while sr != 0 {
        sr = try!(er.read_from(r, buf));
        let mut sw = sr;
        while sw > 0 {
          sw = try!(ew.write_into(w,&buf[..sr]));
          if sw == 0 {
            return Err(IoError::new(IoErrorKind::Other, "Proxying failed, it seems we do not write all content"));
          }
        }
      }
      try!(er.read_end(r));
      try!(ew.write_end(w));
      try!(ew.flush_into(w));
    }

  Ok(())
}


pub struct TunnelProxyExt2<'a, P : Peer, W : 'a + Write, R : 'a + Read> {
  inner : &'a mut TunnelReaderExt2<'a,P,R>,
//  shad :  <P as Peer>::Shadow, // shadow for write not needed for now
  buf: &'a mut [u8],
  modewritten : bool,
  sender: &'a mut W,
}


impl<'a, P : Peer, W : 'a + Write, R : 'a + Read> TunnelProxyExt2<'a, P, W, R> {
  pub fn new(r : &'a mut TunnelReaderExt2<'a,P,R>, buf : &'a mut [u8], s : &'a mut W) -> Self {
    TunnelProxyExt2 {
      inner : r,
      buf : buf,
      modewritten : false,
      sender : s,
    }
  }

  /// iterate on it to empty reader into sender  and flush with additional content (shadow is same kind for reader (ourself) and dest as a tunnel is
  /// using only on kind).
  ///(do not write header and behave according to mode)
  /// Primitive to proxy (could be simple function).
  pub fn tunnel_proxy(&mut self) -> Result<usize> {
    if self.inner.mode.is_none() {
      try!(self.inner.init_read());
    }
    match &self.inner.mode {
      &Some(TunnelMode::NoTunnel) => {
        // imediatly after header the content enc only with dest
        let ix = try!(self.inner.reader.read(self.buf));
        println!("prox:{:?}",&self.buf[..ix]);
        let wix = try!(self.sender.write(&self.buf[..ix]));
        assert!(wix == ix); // or loop if needed
        Ok(wix)

      },
      &Some(TunnelMode::PublicTunnel(nbhop,ref tsmode)) => {
        if !self.modewritten {
          try!(bin_encode(&self.inner.mode.as_ref().unwrap(), self.sender, SizeLimit::Infinite).map_err(|e|BincErr(e)));
          self.modewritten = true;
        }
        let ix = try!(self.inner.reader.read(self.buf));
        let wix = try!(self.sender.write(&self.buf[..ix]));
        assert!(wix == ix); // or loop if needed
        Ok(wix)

      },
      _ => {
        // TODO
        panic!("todo");
      },
    }

  }
}

// create a tunnel writter
// - list of peers (last one is dest)
// - mode : TunnelMode
// - the actual writer (transport most of the time as a mutable reference)

impl<'a, P : Peer, W : 'a + Write> TunnelWriterExt2<'a, P, W> {

/*  pub fn new_test(peers : &[&P], mode : TunnelMode<<<P as Peer>::Shadow as Shadow>::ShadowMode>, sender : &'a mut W) -> Option<TunnelWriterExt2<'a, P, W>> {
    let swol = match mode.get_head_mode() {
      Some(m) => {
        let mut res = ShadowWriteOnceL::new(peers[peers.len() - 1].get_shadower(true),sender, m.clone(), true);
        for i in peers.len() - 1 .. 0 {
  
        }
        Some(res)
      },
      None => {
        None // TODO enum with single write also : simple shadow
      }
    };
    None
  }*/
  pub fn new(peers : &[&P], mode : TunnelMode, buf : &'a mut [u8], sender : &'a mut W, error : Option<usize>, 
  headmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
  contmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
) -> TunnelWriterExt2<'a, P, W> {
    let nbpeer = peers.len();
    let mut addr = Vec::with_capacity(nbpeer);
    let mut shad = Vec::with_capacity(nbpeer);
    let mut hopids = Vec::with_capacity(nbpeer);
    let mut thrng = thread_rng();
    let mut geniter = thrng.gen_iter();

    if let TunnelShadowMode::NoShadow = mode.tunnel_shadow_mode() {
      for p in peers {
        addr.push(p.to_address());
        let mut s = p.get_shadower(true);
        if mode.is_het() {
          s.set_mode(headmode.clone());
        } else {
          s.set_mode(contmode.clone());
        }
        shad.push(s);
        hopids.push(geniter.next().unwrap());
      }
    }
    let shacont = if mode.is_het() {
      peers.get(peers.len() -1).map(|p|{
        let mut s = p.get_shadower(true);
        s.set_mode(contmode.clone());
        s
      })
    } else {
      None
    };
    TunnelWriterExt2 {
      buf: buf,
      bufix: 0,
      hasheader: false,
      addrs: addr,
      shads: shad,
      mode: mode.clone(),
      shacont : shacont,
      sender: sender,
      error: error,
      hopids: hopids,
    }
  }
}

// impl write
impl<'a, P : Peer, W : 'a + Write> Write for TunnelWriterExt2<'a, P, W> {

  fn write(&mut self, cont: &[u8]) -> Result<usize> {
    // first frame : header
    if !self.hasheader {
      // pattern match here to avoid self lifetime constraint
      let &mut TunnelWriterExt2 {
            buf: _,
            bufix: _,
            hasheader: _,
            addrs: ref mut addrs,
            shads: ref mut shads,
            mode: ref mode,
            shacont : ref mut shacont,
            sender: ref mut sender,
            error: ref mut error,
            hopids: ref mut hopids,
      } = self;

      try!(bin_encode(mode, sender, SizeLimit::Infinite).map_err(|e|BincErr(e)));

      match &self.mode.clone() {
        &TunnelMode::NoTunnel => (),
        &TunnelMode::PublicTunnel(_,ref mode) => {

          let mut i = 0;
          // write proxy infos skip first (origin)
          let init : Result<(usize,Option<&mut Write>,Option<ShadowWriteOnceL<<P as Peer>::Shadow>>)> = Ok((i,Some(sender),None));
          try!(shads.iter_mut().fold(init,|pr, sha| match pr {
         e@Err(_) => e,
         Ok((i,ow,olw)) => {
//           let mut fw = None;
         if i != 0 {
            let tunid = hopids[i];
            let npi = if i == addrs.len() - 1 {
              None
            } else {
            let add = addrs.get(i + 1).unwrap();
              Some(add)
            };
//            add.write_as_bytes(&mut shw).unwrap();
            let tpi : TunnelProxyInfoSend<P> = TunnelProxyInfoSend {
              next_proxy_peer : npi,
              tunnel_id : tunid.clone(), // tunnelId change for every hop TODO see if ref in proxy send
              tunnel_id_failure : error.clone(), // if set that is a failure TODO see if ref in proxy send
            };

/*            let mut shw = if ow.is_some() {
              ShadowWriteOnceL::new(sha,ow.unwrap(),headmode,true)
            } else {
              assert!(olw.is_some()); // or algo wrong
              ShadowWriteOnceL::new(sha,olw.as_mut().unwrap(),headmode,false)
            };

            try!(bin_encode(&tpi, &mut shw, SizeLimit::Infinite).map_err(|e|BincErr(e)));

            // write content shad head if needed (encoded)
            if npi.is_none() {
              match shacont {
                &mut Some(ref mut s) => { 
                  try!(s.shadow_header(&mut shw, contentmode)); // shadow header is encrypted
                  try!(shw.flush());
                },
                _ => (),
              }
            } else {
              try!(shw.flush());
            };
            fw = Some(shw);

//            panic!("lifetime issue shw is linked to 410");
// TODO uncoment           oshw = Some(&mut shw);
        }
        Ok((i + 1, None, fw))*/
         }
        Ok((i,ow,olw))
        },
          }));
        },
        &TunnelMode::Tunnel(_,ref mode) => {
          panic!("unimplemented tunnelmode");
//          let hops = 
        },
        &TunnelMode::BiTunnel(_,ref errorasfwd,ref mode) => {
          panic!("unimplemented tunnelmode");
        },
      }



      self.hasheader = true;
    }

    // actual write
    match self.mode {
      TunnelMode::NoTunnel => {
        self.sender.write(cont)
      },
      TunnelMode::PublicTunnel(_,ref mode) => {
        // write cont non layered (public actually only non layered)
        match &mut self.shacont {
          &mut Some(ref mut s) => s.shadow_iter(cont, self.sender),
          _ => {
            let mut sha = self.shads.get_mut(self.addrs.len() - 1).unwrap();
            sha.shadow_iter(cont, self.sender)
          },
        }

        
      },
 
      _ => {
        panic!("unimplemented tunnelmode");
      },
 
    }

  }

  fn flush(&mut self) -> Result<()> {
    match self.mode {
      TunnelMode::NoTunnel => {
      },
      TunnelMode::PublicTunnel(_,ref mode) => {
        // write cont non layered (public actually only non layered)
        let mut sha = self.shads.get_mut(self.addrs.len() - 1).unwrap();
        try!(sha.shadow_flush(self.sender));
      },
      _ => {
        //  write backroute for last hop only (on each proxy back is appended)
        panic!("unimplemented tunnelmode");
      },
 
    }

    // rewrite header next time
    self.hasheader = false;
    self.sender.flush()
  }

}

// create a tunnel reader : use PeerTest (currently using NoShadow...)
impl<'a, P : Peer, R : 'a + Read> TunnelReaderExt2<'a,P,R> {
 
  pub fn new(r : &'a mut R, buf : &'a mut [u8], p : &P,
  headmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
  contmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
  ) -> TunnelReaderExt2<'a,P,R> {
      let mut shad = p.get_shadower(false);
      shad.set_mode(headmode);
      let mut shacont = p.get_shadower(false);
      shacont.set_mode(contmode);
    TunnelReaderExt2 {
      rem_size : 0,
      buf: buf,
      mode: None,
      shadow: shad, // our decoding
      shacont: shacont, // our decoding
      reader: r,
      proxyinfo : None,
    }
  }
  pub fn is_dest(&self) -> Option<bool> {
    if self.mode == Some(TunnelMode::NoTunnel) {
      return Some(true)
    }
    self.proxyinfo.as_ref().map(|pi|pi.next_proxy_peer.is_none())
  }
  pub fn init_read(&mut self) -> Result<()> {
    let tun_mode = try!(bin_decode(&mut self.reader, SizeLimit::Infinite).map_err(|e|BindErr(e)));
    println!("tun mode : {:?}",tun_mode);
    match tun_mode {
       TunnelMode::NoTunnel => (), // nothing to read as header
       TunnelMode::PublicTunnel(nbhop,ref mode) => {
         {
           let tpi = if mode.is_het() {
             let mut shr = new_shadow_read_once(&mut self.reader,&mut self.shadow);
             let tpi = try!(bin_decode(&mut shr, SizeLimit::Infinite).map_err(|e|BindErr(e)));
             try!(self.shacont.read_shadow_header(&mut shr));
             tpi
           } else {
             let mut shr = new_shadow_read_once(&mut self.reader,&mut self.shacont);
             try!(bin_decode(&mut shr, SizeLimit::Infinite).map_err(|e|BindErr(e)))
           };
           self.proxyinfo = Some(tpi);
         }
       },
       _ => {
        panic!("todo");
       },
      }
      self.mode = Some(tun_mode);
      Ok(())
  }

}


// impl read
impl<'a, P : Peer, R : 'a + Read> Read for TunnelReaderExt2<'a,P,R> {
  /// to init frame just read on null buffer (allow subsequent read if terminal)
  fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
    if self.mode.is_none() {
      try!(self.init_read());
    }
    match self.mode {
      Some(TunnelMode::NoTunnel) => {
        self.reader.read(buf)
      },
      Some(TunnelMode::PublicTunnel(nbhop,ref mode)) => {
         self.shacont.read_shadow_iter(&mut self.reader, buf)
      },
      _ => {
        // TODO
        panic!("todo");
      },
    }

  }
}



// test multiple tunnel (one hop) between two thread (similar to real world)

// all test in extra with rsa-peer : by mydht-base-test with RSaPeer


