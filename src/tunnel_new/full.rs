use readwrite_comp::{
  MultiW,
  MultiWExt,
  MultiRExt,
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
use super::{
  TunnelWriter,
  TunnelState,
  Info,
};
use std::io::{
  Write,
  Read,
  Result,
};
use peer::Peer;
use bincode::SizeLimit;
use bincode::rustc_serialize::{
  encode_into as bin_encode, 
  decode_from as bin_decode,
};
use super::BincErr;
use super::TunnelNoRep;
use super::MultipleReplyMode;
use super::TunnelManager;
use super::MultipleReplyInfo;
use super::nope::Nope;
use std::marker::PhantomData;

/// Reply and Error info for full are currently hardcoded MultipleReplyInfo
/// This is not the cas for FullW or FullR, that way only Full need to be reimplemented to use less 
/// generic mode TODO make it generic ? (associated InfoMode type and info constructor)
pub struct Full<P : Peer, E : ExtWrite> {
  pub me : P,
  pub reply_mode : MultipleReplyMode,
  pub error_mode : MultipleReplyMode,
  pub _p : PhantomData<(P,E)>,
}

type Shadows<P : Peer, E : ExtWrite, RI : Info> = CompExtW<MultiWExt<TunnelShadowW<P,RI>>,E>;

/**
 * No impl for instance when no error or no reply
 *
 *
 * Full writer : use for all write and reply, could be split but for now I use old code.
 *
 */
pub struct FullW<RI : Info, EI : Info, P : Peer, E : ExtWrite > {
  state: TunnelState,
  error_info : EI,
  current_cache_id: Option<Vec<u8>>,
  shads: Shadows<P,E,RI>,
}

pub struct FullR {
}

pub struct FullSRW {
}

//impl TunnelNoRep for Full {

impl<P : Peer, E : ExtWrite> TunnelNoRep for Full<P,E> {
  type P = P;
  type TW = FullW<MultipleReplyInfo<P>, MultipleReplyInfo<P>, P, E>;
  type TR = Nope; // TODO

  fn new_reader_no_reply (&mut self, _ : &Self::P) -> Self::TR {
    Nope()
  }
  fn new_writer_no_reply (&mut self, p : &Self::P) -> Self::TW {
    let shads = self.next_shads(p);
    let error_info = self.new_error_info();
    FullW {
      current_cache_id : None,
      state : TunnelState::ReplyOnce,
      error_info : error_info,
      shads: shads,
    }
  }

}

// TODO move fn to TunnelManager
// This should be split for reuse in last or others (base fn todo)
impl<P : Peer, E : ExtWrite> Full<P,E> {

  fn new_error_info (&mut self) -> MultipleReplyInfo<P> {
    match self.error_mode {
      MultipleReplyMode::NoHandling => MultipleReplyInfo::NoHandling,
      MultipleReplyMode::KnownDest => MultipleReplyInfo::KnownDest(self.me.get_key()),
      MultipleReplyMode::Route => MultipleReplyInfo::Route,
      MultipleReplyMode::CachedRoute=> MultipleReplyInfo::CachedRoute(self.new_error_code()),
    }
  }
  fn new_error_code (&mut self) -> usize {
    unimplemented!()
  }
  fn next_shads<RI : Info> (&mut self, p : &P) -> Shadows<P,E,RI> {
    unimplemented!()
  }

  fn new_rep
    <RI : Info, EI : Info> 
    (&self, ri : RI, ei : EI, shads: Shadows<P,E,RI>) -> FullW<RI,EI,P,E> {
    FullW {
      current_cache_id : None,
      state : TunnelState::QueryOnce,
      error_info : ei,
      shads: shads,
    }
  }


  fn new_no_rep
    <RI : Info, EI : Info> 
    (&self, ri : RI, ei : EI, shads: Shadows<P,E,RI>) -> FullW<RI,EI,P,E> {
    FullW {
      current_cache_id : None,
      state : TunnelState::ReplyOnce,
      error_info : ei,
      shads: shads,
    }
  }
/*
  fn new_with_sym
    <RI : Info, EI : Info, P : Peer, E : ExtWrite, TC : TunnelManager<FullSRW>> 
    (&self, ri : RI, ei : EI, shads: Shadows<P,E,RI>, tc : &mut TC) -> FullW<RI,EI,P,E> {
    let comid = if ri.do_cache() || ei.do_cache() {
      let fsrw = FullSRW {    };
      Some(tc.storeW(fsrw))
    } else {
      None
    };
    FullW{
      current_cache_id : comid,
      state : TunnelState::QueryCached,
      error_info : ei,
      shads: shads,
    }
  }*/
}

/// Wrapper over TunnelWriter to aleviate trait usage restrictions, WriterExt is therefore to be
/// implemented on this (see full.rs)
/// This could be removed after specialization
pub struct TunnelWriterFull<TW : TunnelWriter> (TW);

/// TODO this impl must move to all TunnelWriter
impl<TW : TunnelWriter> ExtWrite for TunnelWriterFull<TW> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(self.0.write_state(w));
    try!(self.0.write_connect_info(w));
    /*
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
    }*/

    Ok(())
  }

  #[inline]
  fn write_into<W : Write>(&mut self, _ : &mut W, _ : &[u8]) -> Result<usize> {
    Ok(0)
  }

  #[inline]
  fn write_all_into<W : Write>(&mut self, _ : &mut W, _ : &[u8]) -> Result<()> {
    Ok(())
  }

  #[inline]
  fn flush_into<W : Write>(&mut self, _ : &mut W) -> Result<()> {
    Ok(())
  }

  #[inline]
  fn write_end<W : Write>(&mut self, _ : &mut W) -> Result<()> {
    Ok(())
  }
}

struct TunnelReaderFull;
impl ExtRead for TunnelReaderFull {
  #[inline]
  fn read_header<R : Read>(&mut self, _ : &mut R) -> Result<()> {
    Ok(())
  }

  #[inline]
  fn read_from<R : Read>(&mut self, _ : &mut R, _ : &mut[u8]) -> Result<usize> {
    Ok(0)
  }

  #[inline]
  fn read_exact_from<R : Read>(&mut self, _ : &mut R, _ : &mut[u8]) -> Result<()> { Ok(()) }

  #[inline]
  fn read_end<R : Read>(&mut self, _ : &mut R) -> Result<()> {
    Ok(())
  }
}


impl<E : ExtWrite, P : Peer, RI : Info, EI : Info> TunnelWriter for FullW<RI,EI,P,E> {
  #[inline]
  fn write_state<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(bin_encode(&self.state, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    Ok(())
  }

  #[inline]
  fn write_connect_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
    if let Some(cci) = self.current_cache_id.as_ref() {
      try!(bin_encode(cci, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    }
    Ok(())
  }

 fn write_simkeys_into< W : Write>( &mut self, w : &mut W) -> Result<()> {

  // not all key must be define
  let len : usize = self.shads.0.inner_extwrites().iter().fold(0,|i,item|if item.rep.get_reply_key().is_some() {i+1} else {i});
  try!(bin_encode(&len, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
  let mut key = None;
  // copy/clone each key due to lifetime, this is not optimal
  for i in 0..len {
    match self.shads.0.inner_extwrites().get(i) {
      Some(ref sh) => match sh.rep.get_reply_key() {
        Some(sk) => if key.is_none() {
          key = Some(sk.clone());
        } else {
          key.as_mut().map(|k|k.clone_from(&sk));
        },
        None => key = None,
      },
      None => key = None,
    }
    match key {
      Some(ref k) => try!(self.shads.write_all_into(w, &k)),
      None => (),
    }

  }
  Ok(())
}
}



//////-------------

/// override shadow for tunnel writes (additional needed info)
/// First ExtWrite is bytes_wr to use for proxying content (get end of encoded stream).
/// next is possible shadow key for reply,
/// And last is possible shadowroute for reply or error
pub struct TunnelShadowW<P : Peer, RI : Info> {
  shad : <P as Peer>::Shadow, 
  rep : RI,
}

/// TODO implement Info
/// TODO next split it (here it is MultiReply whichi is previous enum impl, purpose of refacto is
/// getting rid of those enum (only if needed)
pub struct ReplyInfo<E : ExtWrite, P : Peer,TW : TunnelWriter> {
  info : TunnelProxyInfo<P>,
  // this should be seen as a way to establish connection so for new rep, replykey 
  replykey : Option<Vec<u8>>, 
  // reply route should be seen as a reply info : used to write the payload -> TODO redesign this
  replyroute : Option<(E,Box<TunnelWriterFull<TW>>)>,
  //replyroute : Option<Box<(E,TunnelWriterFull<E,P,TW>)>>,
}

impl<E : ExtWrite, P : Peer,TW : TunnelWriter> Info for ReplyInfo<E,P,TW> {
  
  fn do_cache (&self) -> bool {
    self.replykey.is_some()
  }

  fn get_reply_key(&self) -> Option<&Vec<u8>> {
    self.replykey.as_ref()
  }
  fn write_in_header<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    try!(bin_encode(&self.info, inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));

    // write tunnel simkey
    if self.replykey.is_some() {

          //let shadsim = <<P as Peer>::Shadow as Shadow>::new_shadow_sim().unwrap();
      let mut buf :Vec<u8> = Vec::new();
      try!(inw.write_all(&self.replykey.as_ref().unwrap()[..]));
//      try!(self.2.as_mut().unwrap().send_shadow_simkey(&mut inw)); 
/*      let mut cbuf = Cursor::new(buf);
      println!("one");
      try!(self.2.as_mut().unwrap().send_shadow_simkey(&mut cbuf));
 let mut t = cbuf.into_inner();
 println!("{:?}",t);
      inw.write_all(&mut t [..]);
    } else {
      println!("two");*/
    }


    Ok(())
  }

  fn write_after<W : Write>(&mut self, w : &mut W) -> Result<()> {
    match self.replyroute {
      Some((ref mut limiter ,ref mut rr)) => {
        {
          let mut inw  = CompExtWInner(w,limiter);
          // write header (simkey are include in headers)
          try!(rr.write_header(&mut inw));
          // write simkeys to read as dest (Vec<Vec<u8>>)
          // currently same for error
          // put simkeys for reply TODO only if reply expected : move to full reply route ReplyInfo
          // impl -> Reply Info not need to be write into
          try!(rr.0.write_simkeys_into(&mut inw));

          try!(rr.write_end(&mut inw));
        }
        try!(limiter.write_end(w));
        try!(limiter.flush_into(w));
      }
      None => ()
    }
    Ok(())
  }
}

//pub struct TunnelShadowW<E : ExtWrite, P : Peer> (pub <P as Peer>::Shadow, pub TunnelProxyInfo<P>, pub Option<Vec<u8>>, Option<(E,Box<TunnelWriterFull<E,P>>)>);

/// Tunnel proxy info : tunnel info accessible for any peer, the serializable/deser (and pretty
/// common part) of reply info
#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
pub struct TunnelProxyInfo<P : Peer> {
  pub next_proxy_peer : Option<<P as Peer>::Address>,
  pub tunnel_id : usize, // tunnelId change for every hop that is description tci TODO should only be for cached reply info
  pub error_handle : MultipleReplyInfo<P>, // error handle definition
}

impl<P : Peer, RI : Info> ExtWrite for TunnelShadowW<P, RI> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {

    // write basic tunnelinfo and content
    try!(self.shad.write_header(w));
    let mut inw  = CompExtWInner(w, &mut self.shad);


    self.rep.write_in_header(&mut inw);
    Ok(())
  }
  #[inline]
  fn write_into<W : Write>(&mut self, w : &mut W, cont : &[u8]) -> Result<usize> {
    self.shad.write_into(w,cont)
  }   
  #[inline]
  fn flush_into<W : Write>(&mut self, w : &mut W) -> Result<()> {
    self.shad.flush_into(w)
  }
  #[inline]
  fn write_end<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(self.shad.write_end(w));
    // reply or error route TODO having it after had write end is odd (even if at this level we are
    // into another tunnelw shadow), related with read up to reply TODO this might only be ok for
    // error (dif between EI and RI in this case)
    // This is simply that we are added to the end but in various layers.
    // reply or error route TODO having it after had write end is odd (even if at this level we are
    try!(self.rep.write_after(w));

    Ok(())
  }
}



