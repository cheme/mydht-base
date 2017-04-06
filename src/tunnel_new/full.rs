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
  TunnelWriterExt,
  TunnelState,
  TunnelErrorWriter,
  TunnelReplyWriter,
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
use super::MultipleReplyMode;
use super::TunnelCacheManager;
use super::MultipleReplyInfo;

pub struct Full {
  pub reply_mode : MultipleReplyMode,
  pub error_mode : MultipleReplyMode,
}

type Shadows<P : Peer, E : ExtWrite, TW : TunnelWriter<E,P>> = CompExtW<MultiWExt<TunnelShadowW<E, P,TW>>,E>;
/**
 * No impl for instance when no error or no reply
 */
pub struct FullW<RI : Info, EI : Info, P : Peer, E : ExtWrite, TW : TunnelWriter<E,P>> {
  state: TunnelState,
  reply_info : RI,
  error_info : EI,
  current_cache_id: Option<Vec<u8>>,
  shads: Shadows<P,E,TW>,
}

pub struct FullR {
}

pub struct FullSRW {
}

// TODO move fn to TunnelManager
impl Full {

  pub fn new_rep
    <RI : Info, EI : Info, P : Peer, E : ExtWrite, TW : TunnelWriter<E,P>> 
    (&self, ri : RI, ei : EI, shads: Shadows<P,E,TW>) -> FullW<RI,EI,P,E,TW> {
    FullW {
      current_cache_id : None,
      state : TunnelState::QueryOnce,
      reply_info : ri,
      error_info : ei,
      shads: shads,
    }
  }


  pub fn new_no_rep
    <RI : Info, EI : Info, P : Peer, E : ExtWrite, TW : TunnelWriter<E,P>> 
    (&self, ri : RI, ei : EI, shads: Shadows<P,E,TW>) -> FullW<RI,EI,P,E,TW> {
    FullW {
      current_cache_id : None,
      state : TunnelState::ReplyOnce,
      reply_info : ri,
      error_info : ei,
      shads: shads,
    }
  }

  pub fn new_with_sym
    <RI : Info, EI : Info, P : Peer, E : ExtWrite, TW : TunnelWriter<E,P>, TC : TunnelCacheManager<FullSRW>> 
    (&self, ri : RI, ei : EI, shads: Shadows<P,E,TW>, tc : &mut TC) -> FullW<RI,EI,P,E,TW> {
    let comid = if ri.do_cache() || ei.do_cache() {
      let fsrw = FullSRW {    };
      Some(tc.storeW(fsrw))
    } else {
      None
    };
    FullW{
      current_cache_id : comid,
      state : TunnelState::QueryCached,
      reply_info : ri,
      error_info : ei,
      shads: shads,
    }
  }
}


/// TODO this impl must move to all TunnelWriter
impl<E : ExtWrite, P : Peer, TW : TunnelWriter<E, P>> ExtWrite for TunnelWriterExt<E,P,TW> {
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

impl ExtRead for Full {
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


impl<E : ExtWrite, P : Peer, RI : Info, EI : Info, TW : TunnelWriter<E,P>> TunnelWriter<E, P> for FullW<RI,EI,P,E,TW> {
  #[inline]
  fn write_state<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(bin_encode(&self.state, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    Ok(())
  }
  #[inline]
  fn write_error_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(bin_encode(&self.error_info, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    Ok(())
  }
  #[inline]
  fn write_reply_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
      // useless ?? use for query too ??
      try!(bin_encode(&self.reply_info, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
      
      Ok(())
  }
  #[inline]
  fn write_connect_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
    if let Some(cci) = self.current_cache_id.as_ref() {
      try!(bin_encode(cci, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    }
    Ok(())
  }


}

// impl<E : ExtWrite, P : Peer, RI : Info> TunnelReplyWriter<E, P, RI> for FullSRW {}

impl<E : ExtWrite, P : Peer, RI : Info, EI : Info, TW : TunnelWriter<E,P>> TunnelErrorWriter<E, P> for FullW<RI,EI,P,E,TW> {
}




//////-------------

/// override shadow for tunnel writes (additional needed info)
/// First ExtWrite is bytes_wr to use for proxying content (get end of encoded stream).
/// next is possible shadow key for reply,
/// And last is possible shadowroute for reply or error
pub struct TunnelShadowW<E : ExtWrite, P : Peer,TW : TunnelWriter<E,P>> {
  shad : <P as Peer>::Shadow, 
  info : TunnelProxyInfo<P>,
  // this should be seen as a way to establish connection so for new rep -> TODO in reply info to?
  replykey : Option<Vec<u8>>, 
  // reply route should be seen as a reply info : used to write the payload -> TODO redesign this
  replyroute : Option<(E,Box<TunnelWriterExt<E,P,TW>>)>,
  //replyroute : Option<Box<(E,TunnelWriterExt<E,P,TW>)>>,
}
//pub struct TunnelShadowW<E : ExtWrite, P : Peer> (pub <P as Peer>::Shadow, pub TunnelProxyInfo<P>, pub Option<Vec<u8>>, Option<(E,Box<TunnelWriterExt<E,P>>)>);

/// Tunnel proxy info : tunnel info accessible for any peer
#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
pub struct TunnelProxyInfo<P : Peer> {
  pub next_proxy_peer : Option<<P as Peer>::Address>,
  pub tunnel_id : usize, // tunnelId change for every hop that is description tci
  pub tunnel_id_failure : Option<usize>, // if set that is a failure TODO remove (never used)
  pub error_handle : MultipleReplyInfo<P>, // error handle
}

impl<E : ExtWrite, P : Peer, TW : TunnelWriter<E,P>> ExtWrite for TunnelShadowW<E,P, TW> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {

    // write basic tunnelinfo and content
    try!(self.shad.write_header(w));
    let mut inw  = CompExtWInner(w, &mut self.shad);


    try!(bin_encode(&self.info, &mut inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));

    // write tunnel simkey
    if self.replykey.is_some() {

          //let shadsim = <<P as Peer>::Shadow as Shadow>::new_shadow_sim().unwrap();
      let mut buf :Vec<u8> = Vec::new();
      inw.write_all(&self.replykey.as_ref().unwrap()[..]);
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
    //
    match self.replyroute {
      Some((ref mut limiter ,ref mut rr)) => {
        {
          let mut inw  = CompExtWInner(w,limiter);
          // write header (simkey are include in headers)
          try!(rr.write_header(&mut inw));
          // write simkeys to read as dest (Vec<Vec<u8>>)
          // currently same for error
          // put simkeys for reply TODO only if reply expected : move to full reply route ReplyInfo
         // impl -> Reply Info not need to be write into TODO TODO TODO TODO warn simkeys are not
         // reply info it is part of it (this match is reply info
      //--    try!(write_simkeys_into(rr.shads , &mut inw));


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


/**
 * write all simkey from its shads as content (when writing reply as replyonce)
 * */
#[inline]
pub fn write_simkeys_into<P : Peer, E : ExtWrite, TW : TunnelWriter<E,P>, W : Write>(shads : &mut Shadows<P,E,TW>, w : &mut W) -> Result<()> {

  // not all key must be define
  let len : usize = shads.0.inner_extwrites().iter().fold(0,|i,item|if item.replykey.is_some() {i+1} else {i});
  try!(bin_encode(&len, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
  let mut key = None;
  // copy/clone each key due to lifetime, this is not optimal
  for i in 0..len {
    match shads.0.inner_extwrites().get(i) {
      Some(ref sh) => match sh.replykey {
        Some(ref sk) => if key.is_none() {
          key = Some(sk.clone());
        } else {
          key.as_mut().map(|k|k.clone_from(&sk));
        },
        None => key = None,
      },
      None => key = None,
    }
    match key {
      Some(ref k) => try!(shads.write_all_into(w, &k)),
      None => (),
    }

  }
  Ok(())
}


