//!
//!
//! Multi reply info as used in old implementation (minus error mgmt TODO)
//! TODO with cached associated trait on info, could merge with error.rs??
use std::marker::PhantomData;
use super::super::{
  BincErr,
  RepInfo,
  Info,
  TunnelWriter,
  TunnelWriterExt,
  TunnelNoRep,
  ReplyProvider,
  RouteProvider,
  SymProvider,
};
/// wrong use need redesign TODO redesign it on specific trait (not TW as param)
use super::super::full::TunnelWriterFull;
use bincode::SizeLimit;
use bincode::rustc_serialize::{
  encode_into as bin_encode, 
};
use keyval::KeyVal;
use peer::Peer;
use std::io::{
  Write,
  Result,
};
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


/// Possible multiple reply handling implementation
/// Cost of an enum, it is mainly for testing,c
#[derive(RustcDecodable,RustcEncodable,Debug,Clone,PartialEq,Eq)]
pub enum MultipleReplyMode {
  /// do not propagate errors
  NoHandling,
  /// send error to peer with designed mode (case where we can know emitter for dest or other error
  /// handling scheme with possible rendezvous point), TunnelMode is optional and used only for
  /// case where the reply mode differs from the original tunnelmode TODO remove?? (actually use
  /// for norep (public) to set origin for dest)
  KnownDest,
  /// route for error is included, end of payload should be proxyied.
  Route,
  /// route for error is included, end of payload should be proxyied, different route is use
  /// Same as bitunnel from original implementation
  OtherRoute,
  // TODO bitunnel route
  // if route is cached (info in local cache), report error with same route  CachedRoute,
  CachedRoute,
}

/// Error handle info include in frame, also use for reply
/// TODO split as ReplyInfo is split
/// TODO generic not in tunnel
#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
pub enum MultipleReplyInfo<P : Peer> {
  NoHandling,
  KnownDest(<P as KeyVal>::Key), // TODO add reply mode ??
  Route(Vec<u8>), // route headers are to be read afterward, contains sym key
  CachedRoute(Vec<u8>), // contains symkey for peer shadow
}


impl<P : Peer> MultipleReplyInfo<P> {
  pub fn do_cache(&self) -> bool {
    match self {
      &MultipleReplyInfo::CachedRoute(_) => true,
      _ => false,
    }
  }

  pub fn write_in_header<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    try!(bin_encode(self, inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    Ok(())
  }
  pub fn write_after<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    Ok(())
  }

}


/// TODO implement Info
/// TODO next split it (here it is MultiReply whichi is previous enum impl, purpose of refacto is
/// getting rid of those enum (only if needed)
/// TODO get TunnelProxy info next_proxy_peer and tunnel id as PeerInfo and not reply info (after full running)
/// TODO for perf sake should be an enum (at least with the noreply : except for cache impl those
/// ar null (too bug in tunnelshadoww) : the double option is not a good sign too
pub struct ReplyInfo<E : ExtWrite, P : Peer, TW : TunnelWriterExt> {
  pub info : MultipleReplyInfo<P>,
  // reply route should be seen as a reply info : used to write the payload -> TODO redesign this
  // TODO not TunnelWriterFull in box to share with last
  pub replyroute : Option<(E,Box<TW>)>,
  //replyroute : Option<Box<(E,TunnelWriterFull<E,P,TW>)>>,
}

impl<E : ExtWrite, P : Peer,TW : TunnelWriterExt> Info for ReplyInfo<E,P,TW> {
  

  fn write_in_header<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    try!(bin_encode(&self.info, inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));

//    if self.info.do_cache() { 

    // write tunnel simkey
          //let shadsim = <<P as Peer>::Shadow as Shadow>::new_shadow_sim().unwrap();
//      let mut buf :Vec<u8> = Vec::new();
 //     try!(inw.write_all(&self.replykey.as_ref().unwrap()[..]));
//      try!(self.2.as_mut().unwrap().send_shadow_simkey(&mut inw)); 
/*      let mut cbuf = Cursor::new(buf);
      println!("one");
      try!(self.2.as_mut().unwrap().send_shadow_simkey(&mut cbuf));
 let mut t = cbuf.into_inner();
 println!("{:?}",t);
      inw.write_all(&mut t [..]);
    } else {
      println!("two");*/
  //  }


    Ok(())
  }

  fn write_read_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
    if let MultipleReplyInfo::Route(ref k) = self.info {
      try!(bin_encode(k, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    }
    Ok(())
  }
  fn write_after<W : Write>(&mut self, w : &mut W) -> Result<()> {
    // for dest in route and route other mode
    match self.replyroute {
      Some((ref mut limiter ,ref mut rr)) => {
        {
          let mut inw  = CompExtWInner(w,limiter);
          // write header (simkey are include in headers) (contains dest info)
          try!(rr.write_header(&mut inw));
          // Nothing to write, mul key to read are in end of write header of ReplyOnce

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

impl<E : ExtWrite, P : Peer,TW : TunnelWriterExt> RepInfo for ReplyInfo<E,P,TW> {

  /// TODO consider remove
  #[inline]
  fn do_cache (&self) -> bool {
    self.info.do_cache()
  }

  /// TODO remove called once
  fn get_reply_key(&self) -> Option<&Vec<u8>> {
    match self.info {
      MultipleReplyInfo::Route(ref k) => Some(k),
      _ => None,
    }
  }

}

/// TODO E as explicit limiter named trait for readability
pub struct ReplyInfoProvider<E : ExtWrite + Clone, TNR : TunnelNoRep,SSW,SSR, SP : SymProvider<SSW,SSR,TNR::P>, RP : RouteProvider<TNR::P>> {
  pub mode : MultipleReplyMode,
  pub lim : E,
  pub tunrep : TNR,
  // for different reply route
  pub symprov : SP,
  pub routeprov : RP,
  pub _p : PhantomData<(SSW,SSR)>,
}

/// TODO macro inherit??
impl<E : ExtWrite + Clone,P : Peer, TNR : TunnelNoRep<P=P>,SSW,SSR,SP : SymProvider<SSW,SSR,P>,RP : RouteProvider<P>> SymProvider<SSW,SSR,P> for ReplyInfoProvider<E,TNR,SSW,SSR,SP,RP> {

  #[inline]
  fn new_sym_key (&mut self, p : &P) -> Vec<u8> {
    self.symprov.new_sym_key(p)
  }
  #[inline]
  fn new_sym_writer (&mut self, k : Vec<u8>) -> SSW {
    self.symprov.new_sym_writer(k)
  }
  #[inline]
  fn new_sym_reader (&mut self, k : Vec<u8>) -> SSR {
    self.symprov.new_sym_reader(k)
  }

}

impl<E : ExtWrite + Clone,P : Peer,TW : TunnelWriter, W : TunnelWriterExt<TW=TW>, TNR : TunnelNoRep<P=P,W=W,TW=TW>,SSW,SSR,SP : SymProvider<SSW,SSR,P>,RP : RouteProvider<P>> ReplyProvider<P, ReplyInfo<E,P,W>,SSW,SSR> for ReplyInfoProvider<E,TNR,SSW,SSR,SP,RP> {

  /// Error infos bases for peers
  fn new_reply (&mut self, route : &[&P]) -> Vec<ReplyInfo<E,P,W>> {
     let mut res : Vec<ReplyInfo<E,P,W>> = Vec::with_capacity(route.len()-1);
     let l = route.len();
     match self.mode {
       MultipleReplyMode::NoHandling => 
           for i in 1..l {
              res.push(ReplyInfo {
                info : MultipleReplyInfo::NoHandling,
                replyroute : None,
              })
           },
       MultipleReplyMode::KnownDest => {
           for i in 1..l - 1 {
              res.push(ReplyInfo {
                info : MultipleReplyInfo::NoHandling,
                replyroute : None,
              })
           };
           res.push(ReplyInfo {
                info : MultipleReplyInfo::KnownDest(route[0].get_key()),
                replyroute : None,
           });
       },
       MultipleReplyMode::OtherRoute => {
           for i in 1..l - 1 {
              res.push(ReplyInfo {
                info : MultipleReplyInfo::Route(self.new_sym_key(route[i])),
                replyroute : None,
              })
           };
          let rroute = self.tunrep.new_writer_with_route(&self.routeprov.new_reply_route(route[l-1]));
          res.push(ReplyInfo {
            info : MultipleReplyInfo::Route(self.new_sym_key(route[l-1])),
            replyroute : Some((self.lim.clone(), Box::new(rroute))),
          });
       },
  //fn new_writer_no_reply (&mut self, &Self::P) -> Self::W; of TunnelNoRep : include TunnelNoRep
  //and a clonable limiter in mulerrorprovider
       MultipleReplyMode::Route => {
           for i in 1..l - 1 {
              res.push(ReplyInfo {
                info : MultipleReplyInfo::Route(self.new_sym_key(route[i])),
                replyroute : None,
              })
           };

         let mut revroute = Vec::from(route);
         // reverse route to get a reply route
         revroute.reverse();
         // TODO remove ref to tunnelwriterfull (cf error.rs)
         let rroute = self.tunrep.new_writer_with_route(&revroute[..]);
           res.push(ReplyInfo {
                 info : MultipleReplyInfo::Route(self.new_sym_key(route[l-1])),
                 replyroute : Some((self.lim.clone(), Box::new(rroute))),
              });
       },
       MultipleReplyMode::CachedRoute => 
            for i in 1..route.len() {
              // write error code
              res.push(ReplyInfo {
                info : MultipleReplyInfo::CachedRoute(self.new_sym_key(route[i])),
                replyroute : None,
              });
           },
     }
 
/*  pub fn errhandling_infos<P : Peer>(&self, route : &[(usize,&P)], error_route : Option<&[(usize,&P)]>) -> Vec<ErrorHandlingInfo<P>> {
     // dest err handling is used as a possible ack (val)
     let mut res = vec![ErrorHandlingInfo::NoHandling; route.len()];
//     let error_route_len = error_route.map(|er|er.len()).unwrap_or(0);
     
     let need_cached_repkey = if let &TunnelMode::Tunnel(..) = self {
       true 
     } else {false};
     match self {
      &TunnelMode::NoTunnel => (),
      &TunnelMode::BiTunnel(_,_,_, ref b) | &TunnelMode::NoRepTunnel(_,_, ref b) | &TunnelMode::Tunnel(_,_, ref b) => {
        match b {
          &ErrorHandlingMode::NoHandling if !need_cached_repkey => (),
          &ErrorHandlingMode::KnownDest(ref ob) if !need_cached_repkey => {
            res[route.len() - 1] = ErrorHandlingInfo::KnownDest(route[0].1.get_key(), ob.as_ref().map(|b|(**b).clone()));
          },
          &ErrorHandlingMode::ErrorRoute if !need_cached_repkey => {
            for i in 0..route.len() {
              res[i] = ErrorHandlingInfo::ErrorRoute;
            }
          },
          // Tunnel & cached error
          _ => {
            for i in 0..route.len() {

              // write error code
              res[i] = ErrorHandlingInfo::ErrorCachedRoute(route.get(i).unwrap().0);
            }
          },

        }
      },
     };
     res
  }

*/
     res
  }
}

/// specific provider for no rpe
pub struct NoMultiRepProvider;

impl<P : Peer, E : ExtWrite,TW : TunnelWriterExt, SSW,SSR> ReplyProvider<P, ReplyInfo<E,P,TW>,SSW,SSR> for NoMultiRepProvider {
  fn new_reply (&mut self, p : &[&P]) -> Vec<ReplyInfo<E,P,TW>> {
    let mut r = Vec::with_capacity(p.len());
    for _ in 0..p.len() {
      r.push(ReplyInfo {
        info : MultipleReplyInfo::NoHandling,
        replyroute : None,
      });
    }
    r
  }
}
impl<P : Peer, SSW,SSR> SymProvider<SSW,SSR,P> for NoMultiRepProvider {
  fn new_sym_key (&mut self, _ : &P) -> Vec<u8> {
    unimplemented!()
  }
  fn new_sym_writer (&mut self, _ : Vec<u8>) -> SSW {
    unimplemented!()
  }
  fn new_sym_reader (&mut self, _ : Vec<u8>) -> SSR {
    unimplemented!()
  }
}

