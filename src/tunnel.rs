//! tunnel : primitive function for tunnel (for both modes
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
};
use std::io::{
  Write,
  Read,
  Result,
  Error as IoError,
  ErrorKind as IoErrorKind,
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
pub enum TunnelMode<ShadowMode : Clone + Eq> {
  
  NoTunnel,

  /// Tunnel using a single path (forward path = backward path). Parameter is number of hop in the
  /// tunnel (1 is direct).
  /// Error take tunnel backward (error is unencrypted (we do not know originator) and therefore
  /// not explicit : id node could not proxied.
  /// If false, content is only encrypted with dest key (if true full frame is encrypted with every
  /// tunnel peers key). If true it is encrypted with every keys (and temp key for every hop are
  /// transmitted to dest for reply (every hop got it packed to)).
  /// First shadowmode is for routing info
  /// Second is for message or message plus routing info (full enc)
  Tunnel(u8,bool,ShadowMode,ShadowMode),
  /// Tunnel with different path forward and backward.
  /// param is nb of hop.
  /// If false, only dest content is encrypted.
  /// last bool  is 
  /// Error will use forward route like normal tunnel if boolean is true, or backward route (very
  /// likely to fail and result in query timeout) if boolean is false
  BiTunnel(u8,bool,bool,ShadowMode,ShadowMode),
 
  /// in this mode origin and destination know themselve (cf message mode), we simply reply with same nb hop. Back
  /// route is therefore from dest (no need to include back route info.
  PublicTunnel(u8,ShadowMode,ShadowMode),
  

}
impl<SM : Clone + Eq> TunnelMode<SM> {
  pub fn is_full_enc(&self) -> bool {
    match self {
      &TunnelMode::NoTunnel => false,
      &TunnelMode::Tunnel(_,b,_,_) => b,
      &TunnelMode::BiTunnel(_,b,_,_,_) => b,
      &TunnelMode::PublicTunnel(_,_,_) => false,
    }
  }
  /// return head mode only if not the same as main mode and it is actually used
  pub fn get_head_mode(&self) -> Option<&SM> {
    match self {
      &TunnelMode::NoTunnel => None,
      &TunnelMode::Tunnel(_,ref b,ref h,ref m) => {
        if !*b && *h != *m {
          Some(h)
        } else {None}
      },
      &TunnelMode::BiTunnel(_,ref b,_,ref h,ref m) => {
        if !*b && *h != *m {
          Some(h)
        } else {None}
      },
      &TunnelMode::PublicTunnel(_,ref h,ref m) => {
        if *h != *m {
          Some(h)
        } else {None}
      },
    }
  }
  pub fn get_main_mode(&self) -> Option<&SM> {
    match self {
      &TunnelMode::NoTunnel => None,
      &TunnelMode::Tunnel(_,_,_,ref m) => Some(m),
      &TunnelMode::BiTunnel(_,_,_,_,ref m) => Some(m),
      &TunnelMode::PublicTunnel(_,_,ref m) => Some(m),
    }
  }

  // do we use distinct encoding for content
  pub fn is_het(&self) -> bool {
    match self {
      &TunnelMode::NoTunnel => false,
      &TunnelMode::Tunnel(_,_,ref h,ref m) => h != m,
      &TunnelMode::BiTunnel(_,_,_,ref h,ref m) => h != m,
      &TunnelMode::PublicTunnel(_,ref h,ref m) => h != m,
    }
  
  }

}
#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
/// QueryMode info to use in message between peers.
/// TODO delete : it is info that are read/write directly in methods (tunnelid)
pub enum TunnelModeMsg<ShadowMode : Clone + Eq> {
  Tunnel(u8,Option<usize>, TunnelID, ShadowMode, ShadowMode), //first u8 is size of tunnel for next hop of query, usize is size of descriptor for proxying info, if no usize it means it is fully shadowed and a pair is under it
  BiTunnel(u8,bool,Option<usize>,TunnelID, ShadowMode, ShadowMode), // see tunnel, plus if bool is true we error reply with forward route stored as laste param TODO replace Vec<u8> by Reader (need encodable of this reader as writalll
  PublicTunnel(u8,usize,TunnelID,ShadowMode,ShadowMode),
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

impl<ShadowMode : Clone + Eq> TunnelModeMsg<ShadowMode> {
  /// get corresponding querymode
  pub fn get_mode (&self) -> TunnelMode<ShadowMode> {
    match self {

      &TunnelModeMsg::Tunnel (l,Some(_),_,ref s1,ref s2) => TunnelMode::Tunnel(l,false,s1.clone(),s2.clone()),
      &TunnelModeMsg::Tunnel (l,None,_,ref s1,ref s2) => TunnelMode::Tunnel(l,true,s1.clone(),s2.clone()),
      &TunnelModeMsg::BiTunnel (l,f,Some(_),_,ref s1,ref s2) => TunnelMode::BiTunnel(l,false,f,s1.clone(),s2.clone()),
      &TunnelModeMsg::BiTunnel (l,f,None,_,ref s1,ref s2) => TunnelMode::BiTunnel(l,true,f,s1.clone(),s2.clone()),
      &TunnelModeMsg::PublicTunnel(l,_,_,ref s1, ref s2) => TunnelMode::PublicTunnel(l,s1.clone(),s2.clone()),
    }
   
  }
  pub fn get_tunnelid (&self) -> &TunnelID {
    match self {
      &TunnelModeMsg::Tunnel (_,_,ref tid,_,_) => tid,
      &TunnelModeMsg::BiTunnel (_,_,_,ref tid,_,_) => tid,
      &TunnelModeMsg::PublicTunnel(_,_,ref tid,_, _) => tid,
    }
  }
  pub fn get_tunnel_length (&self) -> u8 {
    match self {
      &TunnelModeMsg::Tunnel (l,_,_,_,_) => l,
      &TunnelModeMsg::BiTunnel (l,_,_,_,_,_) => l,
      &TunnelModeMsg::PublicTunnel(l,_,_,_, _) => l,
    }
  }
  pub fn get_shadow_proxy_info (&self) -> ShadowMode {
    match self {
      &TunnelModeMsg::Tunnel (_,_,_,ref s,_) => s.clone(),
      &TunnelModeMsg::BiTunnel (_,_,_,_,ref s,_) => s.clone(),
      &TunnelModeMsg::PublicTunnel(_,_,_,ref s, _) => s.clone(),
    }
  }
  pub fn get_shadow_message (&self) -> ShadowMode {
    match self {
      &TunnelModeMsg::Tunnel (_,_,_,_,ref s) => s.clone(),
      &TunnelModeMsg::BiTunnel (_,_,_,_,_,ref s) => s.clone(),
      &TunnelModeMsg::PublicTunnel(_,_,_,_,ref s) => s.clone(),
    }
  }

}



/// a reader for a proxy payload
pub struct TunnelReader<'a, P : Peer, R : 'a + Read> {
  // payload remaining size
  rem_size : usize,
  buf: &'a mut [u8],
  mode: Option<TunnelMode<<<P as Peer>::Shadow as Shadow>::ShadowMode>>, // mode is read from first frame
  shadow: <P as Peer>::Shadow, // our decoding
  shacont: <P as Peer>::Shadow, // our decoding for corntent if het
  reader: &'a mut R,
  proxyinfo : Option<TunnelProxyInfo<P>>, // is initialized after first read
}

/// writer to encode for tunneling, it create the header and encode the content
/// Note that all peers are of the same kind (same for shadow) as we currently support only one
/// Peer in one dht (no fat pointer here)
pub struct TunnelWriter<'a, P : Peer, W : 'a + Write> {
  buf: &'a mut [u8],
  bufix: usize,
  addrs: Vec<<P as Peer>::Address>, // of none it means that heading hasbeen sent otherwise it need to be sent
  shads: Vec<<P as Peer>::Shadow>, // of none it means that heading hasbeen sent otherwise it need to be sent
  shacont: Option<<P as Peer>::Shadow>, // shadow for content when heterogenous enc
  hopids: Vec<usize>, // Id of all hop to report error
  error: Option<usize>, // possible error hop to report
  hasheader: bool,
  mode: TunnelMode<<<P as Peer>::Shadow as Shadow>::ShadowMode>,
  sender: &'a mut W,
}

/// iterate on it to empty reader into sender  and flush with additional content (shadow is same kind for reader (ourself) and dest as a tunnel is
/// using only on kind).
///(do not write header and behave according to mode)
/// Primitive to proxy (could be simple function)
pub struct TunnelProxy<'a, P : Peer, W : 'a + Write, R : 'a + Read> {
  inner : &'a mut TunnelReader<'a,P,R>,
//  shad :  <P as Peer>::Shadow, // shadow for write not needed for now
  buf: &'a mut [u8],
  modewritten : bool,
  sender: &'a mut W,
}


impl<'a, P : Peer, W : 'a + Write, R : 'a + Read> TunnelProxy<'a, P, W, R> {
  pub fn new(r : &'a mut TunnelReader<'a,P,R>, buf : &'a mut [u8], s : &'a mut W) -> Self {
    TunnelProxy {
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
      &Some(TunnelMode::PublicTunnel(nbhop,ref contenthead, ref contentshad)) => {
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

impl<'a, P : Peer, W : 'a + Write> TunnelWriter<'a, P, W> {
  pub fn new(peers : &[&P], mode : TunnelMode<<<P as Peer>::Shadow as Shadow>::ShadowMode>, buf : &'a mut [u8], sender : &'a mut W, error : Option<usize>) -> TunnelWriter<'a, P, W> {
    let nbpeer = peers.len();
    let shad_main_mode = mode.get_main_mode();
    let mut addr = Vec::with_capacity(nbpeer);
    let mut shad = Vec::with_capacity(nbpeer);
    let mut hopids = Vec::with_capacity(nbpeer);
    let mut thrng = thread_rng();
    let mut geniter = thrng.gen_iter();
    if shad_main_mode.is_some() {
      for p in peers {
        addr.push(p.to_address());
        shad.push(p.get_shadower(true));
        hopids.push(geniter.next().unwrap());
      }
    }
    let shacont = if mode.is_het() {
      peers.get(peers.len() -1).map(|p|p.get_shadower(true))
    } else {
      None
    };
    TunnelWriter {
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
impl<'a, P : Peer, W : 'a + Write> Write for TunnelWriter<'a, P, W> {

  fn write(&mut self, cont: &[u8]) -> Result<usize> {
    // first frame : header
    if !self.hasheader {
      try!(bin_encode(&self.mode, &mut self.sender, SizeLimit::Infinite).map_err(|e|BincErr(e)));
      match &self.mode {
        &TunnelMode::NoTunnel => {
            self.hasheader = true;
        },
        &TunnelMode::PublicTunnel(_,ref headmode,ref contentmode) => {
          // write proxy infos skip first (origin)
          for i in 1 .. self.addrs.len() {
            let mut sha = self.shads.get_mut(i).unwrap();
            let tunid = self.hopids.get(i).unwrap();
            let npi = if i == self.addrs.len() - 1 {
              None
            } else {
            let add = self.addrs.get(i + 1).unwrap();
              Some(add)
            };
//            add.write_as_bytes(&mut shw).unwrap();
            let tpi : TunnelProxyInfoSend<P> = TunnelProxyInfoSend {
              next_proxy_peer : npi,
              tunnel_id : tunid.clone(), // tunnelId change for every hop
              tunnel_id_failure : self.error, // if set that is a failure
            };

            let mut shw = ShadowWriteOnce::new(sha,self.sender,headmode);
            try!(bin_encode(&tpi, &mut shw, SizeLimit::Infinite).map_err(|e|BincErr(e)));
            // write content shad head if needed (encoded)
            if npi.is_none() {
              match &mut self.shacont {
                &mut Some(ref mut s) => { 
                  try!(s.shadow_header(&mut shw, contentmode));
                  try!(shw.flush());
                },
                _ => (),
              }
//              let mut sha = self.shads.get_mut(self.addrs.len() - 1).unwrap();
            } else {
              try!(shw.flush());
            }
          }

          self.hasheader = true;
        },
        &TunnelMode::Tunnel(_,layercontent,ref headmode,ref contentmode) => {
          if layercontent { panic!("unimplemented layered content") };
          panic!("unimplemented tunnelmode");
//          let hops = 
        },
        &TunnelMode::BiTunnel(_,layercontent,errorasfwd,ref headmode,ref contentmode) => {
          if layercontent { panic!("unimplemented layered content") };
          panic!("unimplemented tunnelmode");
        },
      }
    }

    // actual write
    match self.mode {
      TunnelMode::NoTunnel => {
        self.sender.write(cont)
      },
      TunnelMode::PublicTunnel(_,_,ref contentmode) => {
        // write cont non layered (public actually only non layered)
        match &mut self.shacont {
          &mut Some(ref mut s) => s.shadow_iter(cont, self.sender, contentmode),
          _ => {
            let mut sha = self.shads.get_mut(self.addrs.len() - 1).unwrap();
            sha.shadow_iter(cont, self.sender, contentmode)
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
      TunnelMode::PublicTunnel(_,_,ref contentmode) => {
        // write cont non layered (public actually only non layered)
        let mut sha = self.shads.get_mut(self.addrs.len() - 1).unwrap();
        try!(sha.shadow_flush(self.sender, contentmode));
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
impl<'a, P : Peer, R : 'a + Read> TunnelReader<'a,P,R> {
  pub fn new(r : &'a mut R, buf : &'a mut [u8], shad : <P as Peer>::Shadow, shacont : <P as Peer>::Shadow) -> TunnelReader<'a,P,R> {
    TunnelReader {
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
       TunnelMode::PublicTunnel(nbhop,ref headmode, ref contentmode) => {
         {
           let tpi = if headmode != contentmode {
             let mut shr = ShadowReadOnce::new(&mut self.shadow,&mut self.reader);
             let tpi = try!(bin_decode(&mut shr, SizeLimit::Infinite).map_err(|e|BindErr(e)));
             try!(self.shacont.read_shadow_header(&mut shr));
             tpi
           } else {
             let mut shr = ShadowReadOnce::new(&mut self.shacont,&mut self.reader);
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
impl<'a, P : Peer, R : 'a + Read> Read for TunnelReader<'a,P,R> {
  /// to init frame just read on null buffer (allow subsequent read if terminal)
  fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
    if self.mode.is_none() {
      try!(self.init_read());
    }
    match self.mode {
      Some(TunnelMode::NoTunnel) => {
        self.reader.read(buf)
      },
      Some(TunnelMode::PublicTunnel(nbhop,ref contenthead, ref contentshad)) => {
         self.shacont.read_shadow_iter(&mut self.reader, buf, contentshad)
      },
      _ => {
        // TODO
        panic!("todo");
      },
    }

  }
}

// iterate on tunnelproxy up to emptied (return Result<usize>)


// test from writer to reader directly (one tunnel length)


// test from writer to reader through proxy (one tunnel length) : rem previous test??


// test multiple tunnel


// test multiple tunnel (one hop) between two thread (similar to real world)

// all test in extra with rsa-peer : by mydht-base-test with RSaPeer

