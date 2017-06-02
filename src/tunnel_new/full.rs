
use std::rc::Rc;
use std::cell::RefCell;
use rand::ThreadRng;
use rand::thread_rng;
use rand::Rng;
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
  ChainExtRead,
  DefaultID,
};
use std::io::{
  Write,
  Read,
  Cursor,
  Result,
  Error as IoError,
  ErrorKind as IoErrorKind,
};
use keyval::KeyVal; // Trait actually use TODO find pragma for it
use peer::Peer;
use bincode::SizeLimit;
use bincode::rustc_serialize::{
  encode_into as bin_encode, 
  decode_from as bin_decode,
};
use super::{
  TunnelWriter,
  TunnelReader,
  TunnelReaderError,
  TunnelReaderNoRep,
  TunnelWriterExt,
  TunnelReaderExt,
  TunnelError,
  Info,
  RepInfo,
  BincErr,
  BindErr,
  TunnelNoRep,
  Tunnel,
  TunnelManager,
  TunnelCache,
  SymProvider,
  ErrorProvider,
  ReplyProvider,
  RouteProvider,
};
use super::common::{
  TunnelState,
};
use super::info::multi::{
  MultipleReplyMode,
  MultipleReplyInfo,
};
use super::info::error::{
  MultipleErrorInfo,
  MultipleErrorMode,
};
use super::nope::Nope;
use std::marker::PhantomData;












/// Generic Tunnel Traits, use as a traits container for a generic tunnel implementation
/// (to reduce number of trait parameter), it is mainly here to reduce number of visible trait
/// parameters in code, 
/// TODO reply info and error generic !!! after test for full ok (at least)
pub trait GenTunnelTraits {
  type P : Peer;
  /// Reply frame limiter (specific to use of reply once with header in frame
  type LW : ExtWrite + Clone; // limiter
  type LR : ExtRead + Clone; // limiter
  type SSW : ExtWrite;// symetric writer
  type SSR : ExtRead;// seems userless (in rpely provider if needed)
  type TC : TunnelCache<TunnelCachedWriterExtClone<Self::SSW,Self::LW>,TunnelCachedReaderExtClone<Self::SSR,Self::LR>>;

//  type SP : SymProvider<Self::SSW,Self::SSR>; use replyprovider instead
  type RP : RouteProvider<Self::P>;
  /// Reply writer use only to include a reply envelope
  type RW : TunnelWriterExt;
  type REP : ReplyProvider<Self::P, MultipleReplyInfo<Self::P>,Self::SSW,Self::SSR>;
  type TNR : TunnelNoRep<P=Self::P,W=Self::RW>;
  type EP : ErrorProvider<Self::P, MultipleErrorInfo>;
}

/// Reply and Error info for full are currently hardcoded MultipleReplyInfo
/// This is not the cas for FullW or FullR, that way only Full need to be reimplemented to use less 
/// generic mode TODO make it generic ? (associated InfoMode type and info constructor)
/// TODO remove E ?? or put in GenTunnel (not really generic
/// TODO multiplereplymode generic
pub struct Full<TT : GenTunnelTraits> {
  pub me : TT::P,
  pub reply_mode : MultipleReplyMode,
  pub error_mode : MultipleErrorMode,
  pub cache : TT::TC,
//  pub sym_prov : TT::SP,
  pub route_prov : TT::RP,
  pub reply_prov : TT::REP,
  pub tunrep : TT::TNR,
  pub error_prov : TT::EP,
  pub rng : ThreadRng,
  pub limiter_proto_w : TT::LW,
  pub limiter_proto_r : TT::LR,
  pub _p : PhantomData<TT>,
}

type Shadows<P : Peer, RI : Info, EI : Info,LW : ExtWrite,TW : TunnelWriterExt> = CompExtW<MultiWExt<TunnelShadowW<P,RI,EI,LW,TW>>,LW>;

/**
 * No impl for instance when no error or no reply
 *
 *
 * Full writer : use for all write and reply, could be split but for now I use old code.
 *
 */
pub struct FullW<RI : RepInfo, EI : Info, P : Peer, LW : ExtWrite,TW : TunnelWriterExt > {
  state: TunnelState,
  /// Warning if set it means cache is use, it is incompatible with a non cache tunnel state
  /// (reading will fail if state is not right, so beware when creating FullW)
  current_cache_id: Option<Vec<u8>>,
  shads: Shadows<P,RI,EI,LW,TW>,
}

/**
 * reply to both fullw or proxy headers and act as a single symetric of TunnelShadowW
 *
 * Similar to old TunnelShadowExt impl.
 */
pub struct FullR<RI : RepInfo, EI : Info, P : Peer, LR : ExtRead> {
  state: TunnelState,
  // TODO try moving in proxy or dest read new method (may be the last info written so no need to be here)
  current_cache_id: Option<Vec<u8>>,
  current_reply_info: Option<RI>,
  current_error_info: Option<EI>,
  next_proxy_peer : Option<<P as Peer>::Address>,
  tunnel_id : Option<usize>, // TODO useless remove??
  shad : CompExtR<<P as Peer>::Shadow,LR>,
  content_limiter : LR,
// TODO should be remove when TunnelReader Trait refactor or removed (code for reading will move in new dest
// reader init from tunnel ) : TODO remove !!! put in dest full kind multi while reading or befor
// caching Also true for cacheid. -> require change of trait : init of dest reader with Read as
// param
  dest_read_keys : Option<Vec<Vec<u8>>>,
}

/// keep reference to reader while destreader or proxying
pub struct ProxyFull<OR : ExtRead,SW : ExtWrite, E : ExtWrite, LR : ExtRead> {
  pub origin_read : OR,
  pub kind : ProxyFullKind<SW,E,LR>
}
impl<OR : ExtRead,SW : ExtWrite, E : ExtWrite, LR : ExtRead> TunnelReaderExt for ProxyFull<OR,SW,E,LR> {
  type TR = OR; 
  fn get_reader(self) -> Self::TR {
    self.origin_read
  }
}

impl<OR : ExtRead,SW : ExtWrite, E : ExtWrite, LR : ExtRead> ExtRead for ProxyFull<OR,SW,E,LR> {
  fn read_header<R : Read>(&mut self, r : &mut R) -> Result<()> {
    if let ProxyFullKind::ReplyCached(_,ref mut lr) = self.kind {
      try!(self.origin_read.read_end(r));
      try!(lr.read_header(r))
    };
    // actually already called
    Ok(())
  }

  fn read_from<R : Read>(&mut self, r : &mut R, buf : &mut[u8]) -> Result<usize> {
    match self.kind {
      ProxyFullKind::ReplyCached(_, ref mut rs) => rs.read_from(r,buf),
      ProxyFullKind::QueryOnce(_,_) | ProxyFullKind::QueryCached(_,_) => self.origin_read.read_from(r,buf),
      ProxyFullKind::ReplyOnce(_,ref mut limr,ref mut b,_,_,_) if *b => {
        let mut cr = self.origin_read.chain(limr);
        let i = cr.read_from(r,buf)?;
        if cr.in_second() {
          *b = false;
        }
        Ok(i)
      },
      ProxyFullKind::ReplyOnce(_,ref mut limr,_,_,_,_) => limr.read_from(r,buf),
    }
  }

  fn read_exact_from<R : Read>(&mut self, r : &mut R, mut buf: &mut[u8]) -> Result<()> {
    match self.kind {
      ProxyFullKind::ReplyCached(_, ref mut rs) => rs.read_exact_from(r, buf),
      ProxyFullKind::QueryOnce(_,_) | ProxyFullKind::QueryCached(_,_) => self.origin_read.read_exact_from(r,buf),
      ProxyFullKind::ReplyOnce(_,ref mut limr,ref mut b,_,_,_) if *b => {
        let mut cr = self.origin_read.chain(limr);
        let i = cr.read_exact_from(r,buf)?;
        if cr.in_second() {
          *b = false;
        }
        Ok(i)
      },
      ProxyFullKind::ReplyOnce(_,ref mut limr,_,_,_,_) => limr.read_exact_from(r,buf),
      /*
      ProxyFullKind::ReplyOnce(_,ref mut limr,_,_,ref mut b) if *b => {
        // TODO replace it by library chain two read + test cases
        let mut br = &mut buf[..];
        while br.len() > 0 {
          if *b {
        let i = try!(self.origin_read.read_from(r,br));
        br = &mut buf[i..];
        if i == 0 {
          try!(self.origin_read.read_end(r));
          try!(limr.read_header(r));
          *b = false;
          let n = limr.read_from(r,buf)?;
          br = &mut buf[n..];
        } 
        } else {
          let n = limr.read_from(r,buf)?;
          br = &mut buf[n..];
        }
      };
        Ok(())
    },
      ProxyFullKind::ReplyOnce(_,ref mut limr,_,_,_) => limr.read_exact_from(r,buf),*/
    }
  }

  fn read_end<R : Read>(&mut self, r : &mut R) -> Result<()> {
    match self.kind {
      ProxyFullKind::ReplyCached(_,ref mut rs) => rs.read_end(r),
      ProxyFullKind::QueryOnce(_,_) | ProxyFullKind::QueryCached(_,_) => self.origin_read.read_end(r),
      ProxyFullKind::ReplyOnce(_,ref mut limr,true,_,_,_) => {
        let mut cr = self.origin_read.chain(limr);
        cr.read_end(r)
      }
      ProxyFullKind::ReplyOnce(_,ref mut limr,ref mut b,_,_,_) => {
        *b = true;
        limr.read_end(r)
      }
    }
  }
}

/// kind of proxying
/// or nothing proxy as previously read (continue reading from origin read)
/// TODO proxy error kind ?? yes!!
pub enum ProxyFullKind<SW : ExtWrite, LW : ExtWrite, LR : ExtRead> {
  /// end original read done in proxy init (TODO) and proxy as is then proxy content with symetric enc aka ReplyCached
  ReplyCached(TunnelCachedWriterExtClone<SW,LW>,LR),
  /// continue reading from original read and write as is, add cache id if the state if info from
  /// cache or added to cache : aka queryonce, 
  /// TODO remove TunnelState (only one val)
  QueryOnce(TunnelState, LW),
  /// proxy content after with sim writer aka ReplyOnce
  /// TODO remove TunnelStarte (only one val)
  ReplyOnce(TunnelState,LR,bool,CompExtW<SW,LW>,LW,bool),
  /// after putting in cache : aka querycache : same as id plus our cach
  QueryCached(Vec<u8>, LW),
}
 
//impl TunnelNoRep for Full {

impl<TT : GenTunnelTraits> TunnelNoRep for Full<TT> {
  type P = TT::P;
  type W = FullW<MultipleReplyInfo<TT::P>, MultipleErrorInfo, TT::P, TT::LW,TT::RW>;
  type TR = FullR<MultipleReplyInfo<TT::P>, MultipleErrorInfo, TT::P, TT::LR>;
  /// actual proxy writer : TODO rem like W : directly return writer default impl when stabilized
  type PW = ProxyFull<Self::TR,TT::SSW,TT::LW,TT::LR>;
  /// Dest reader
  type DR = DestFull<Self::TR,TT::SSR,TT::LR>;
 
  fn new_reader (&mut self) -> Self::TR {

    let s = self.me.get_shadower(false);
    FullR {
      state: TunnelState::TunnelState,
      current_cache_id: None,
      current_reply_info: None,
      current_error_info: None,
      next_proxy_peer : None,
      tunnel_id : None, // TODO useless remove??
      shad : CompExtR(s,self.limiter_proto_r.clone()),
      content_limiter : self.limiter_proto_r.clone(),
      dest_read_keys : None,
    }


  }
  #[inline]
  fn new_writer (&mut self, p : &Self::P) -> Self::W {
    let state = self.get_write_state();
    let ccid = self.make_cache_id(state.clone());
    let shads = self.next_shads(p, state.clone());
    FullW {
      current_cache_id : ccid,
      state : state,
      shads: shads,
    }
  }
  #[inline]
  fn new_writer_with_route (&mut self, route : &[&Self::P]) -> Self::W {
  let state = self.get_write_state();
    let ccid = self.make_cache_id(state.clone());
    let shads = self.make_shads(route, state.clone());
    FullW {
      current_cache_id : ccid,
      state : state,
      shads: shads,
    }
  }

  fn new_proxy_writer (&mut self, mut or : Self::TR) -> Result<Self::PW> {
    let pfk = match or.state {
      TunnelState::ReplyOnce => {
        let key = or.current_reply_info.as_ref().unwrap().get_reply_key().unwrap().clone();
        let ssw = CompExtW(self.reply_prov.new_sym_writer (key),self.limiter_proto_w.clone());
        ProxyFullKind::ReplyOnce(or.state.clone(), self.limiter_proto_r.clone(),true,ssw,self.limiter_proto_w.clone(),true)
      },
      TunnelState::QueryCached => {
        let osk = or.current_cache_id;
        or.current_cache_id = None;
        let fsk = osk.unwrap();

        let key = or.current_reply_info.as_ref().unwrap().get_reply_key().unwrap().clone();

        let cache_key = self.new_cache_id();
        let ssw = self.new_sym_writer (key,fsk);
        try!(self.put_symw(&cache_key[..],ssw));
        ProxyFullKind::QueryCached(cache_key, self.limiter_proto_w.clone())
      },
      TunnelState::ReplyCached => {

        let tcw = self.get_symw(or.current_cache_id.as_ref().unwrap());

        ProxyFullKind::ReplyCached(tcw.unwrap().clone(),self.limiter_proto_r.clone())
      },
      TunnelState::QueryOnce => {
        ProxyFullKind::QueryOnce(TunnelState::QueryOnce, self.limiter_proto_w.clone())
      },
      TunnelState::TunnelState => {
        // TODO remove this state or unimplement
        ProxyFullKind::QueryOnce(TunnelState::TunnelState, self.limiter_proto_w.clone())
      },

      TunnelState::QError | TunnelState::QErrorCached => unimplemented!(),

    };
    Ok(ProxyFull {
        origin_read : or,
        kind : pfk,
      }
    )

  }
  fn new_dest_reader (&mut self, mut or : Self::TR) -> Result<Self::DR> {
    match or.state {
      TunnelState::TunnelState => {
        // TODO return error of invalid reader state instead of panic
        panic!("invalid reader state")
      },
      TunnelState::QueryOnce => {
        Ok(DestFull {
          origin_read : or,
          kind : DestFullKind::Id,
        })
      },
      TunnelState::QueryCached => {
        // same as query once because no bidirect cache yet (only replycache route use cache and we
        // do not resend) and previous cache id in origin read
        Ok(DestFull {
          origin_read : or,
          kind : DestFullKind::Id,
        })
      },
      TunnelState::ReplyOnce => {

        let ks = or.dest_read_keys.unwrap(); // TODO clean state error on None
        or.dest_read_keys = None; // warning init once semantic would be clearer if read is passed here
        let cr = new_dest_cached_reader_ext(ks.into_iter().map(|k|self.reply_prov.new_sym_reader(k)).collect(), self.limiter_proto_r.clone());
        Ok(DestFull {
          origin_read : or,
          kind : DestFullKind::Multi(cr),
        })
      },
      TunnelState::ReplyCached => {
        unimplemented!() // TODO DestFullKind from cach : may need a new type to Rc<refCell -> DestFullKind::MultiCached
      },
      TunnelState::QError => {
        unimplemented!()
      },
      TunnelState::QErrorCached => {
        unimplemented!()
      },
    }
  }
}

impl<TT : GenTunnelTraits> Tunnel for Full<TT> {
  // reply info info needed to established conn
  type RI = MultipleReplyInfo<TT::P>;
  /// no info for a reply on a reply (otherwhise establishing sym tunnel seems better)
  type RW = FullW<Nope, MultipleErrorInfo, TT::P, TT::LW,TT::RW>;
//pub struct FullW<RI : RepInfo, EI : Info, P : Peer, LW : ExtWrite,TW : TunnelWriterExt > {
  
  fn new_reply_writer (&mut self, p : &Self::P, ri : &Self::RI) -> Self::RW {
    // TODO fn reply as an extwriter with ri as param may need to change RTW type
    unimplemented!()
  }
}


impl<TT : GenTunnelTraits> TunnelError for Full<TT> {
  // TODO use a lighter error info type!!!
  type EI = MultipleErrorInfo;
  /// can error on error, cannot reply also, if we need to reply or error on error that is a Reply 
  /// TODO make a specific type for error reply : either cached or replyinfo for full error
  type EW = Nope;
  fn new_error_writer (&mut self, p : &Self::P, ei : &Self::EI) -> Self::EW {
    
    if let MultipleErrorMode::CachedRoute = self.error_mode {
      let state = TunnelState::QErrorCached;
      // cached error TODO
    } else {
      let peers : Vec<(usize,&TT::P)> = vec!(); // do not know how to get, from read ?? cf fn report_error (change proto)
      // multireply error
      let state = TunnelState::QError;
 
      for i in 0 .. peers.len() -1 {
      }
      panic!("TODO"); // TODO unclear in orignal impl and TODO in test
    }
    unimplemented!();
  }
}

/// Tunnel which allow caching, and thus establishing connections
impl<TT : GenTunnelTraits> TunnelManager for Full<TT> {

  // Shadow Sym (if established con)
  type SSCW = TunnelCachedWriterExtClone<TT::SSW,TT::LW>;
  // Shadow Sym (if established con)
  type SSCR = TunnelCachedReaderExtClone<TT::SSR,TT::LR>;

  fn put_symw(&mut self, k : &[u8], w : Self::SSCW) -> Result<()> {
    self.cache.put_symw_tunnel(k,w)
  }

  fn get_symw(&mut self, k : &[u8]) -> Result<Self::SSCW> {
    let r = try!(self.cache.get_symw_tunnel(k));
    Ok(r.clone())
  }
  fn put_symr(&mut self, w : Self::SSCR) -> Result<Vec<u8>> {
    self.cache.put_symr_tunnel(w)
  }

  fn get_symr(&mut self, k : &[u8]) -> Result<Self::SSCR> {
    let r = try!(self.cache.get_symr_tunnel(k));
    Ok(r.clone())
  }

  fn use_sym_exchange (ri : &Self::RI) -> bool {
    ri.do_cache()
  }

  fn new_sym_writer (&mut self, sk : Vec<u8>, p_cache_id : Vec<u8>) -> Self::SSCW {
    Rc::new(RefCell::new(TunnelCachedWriterExt::new(self.reply_prov.new_sym_writer(sk), p_cache_id, self.limiter_proto_w.clone())))
  }

  fn new_dest_sym_reader (&mut self, ks : Vec<Vec<u8>>) -> Self::SSCR {
    Rc::new(RefCell::new(new_dest_cached_reader_ext(ks.into_iter().map(|k|self.reply_prov.new_sym_reader(k)).collect(), self.limiter_proto_r.clone())))
  }

  fn new_cache_id (&mut self) -> Vec<u8> {
    self.cache.new_cache_id()
  }

}


// This should be split for reuse in last or others (base fn todo)
impl<TT : GenTunnelTraits> Full<TT> {

  /// get state for writing depending on reply
  fn get_write_state (&self) -> TunnelState {
    match self.reply_mode {
      MultipleReplyMode::NoHandling => TunnelState::QueryOnce,
      MultipleReplyMode::KnownDest => TunnelState::QueryOnce,
      MultipleReplyMode::Route => TunnelState::QueryOnce,
      MultipleReplyMode::OtherRoute => TunnelState::QueryOnce,
      MultipleReplyMode::CachedRoute => TunnelState::QueryCached,
      MultipleReplyMode::RouteReply => TunnelState::ReplyOnce,
    }
  }

  // TODO fuse with make_shads (lifetime issue on full : need to open it but first finish
  // make_shads fn
  fn next_shads (&mut self, p : &TT::P, state : TunnelState) -> Shadows<TT::P,MultipleReplyInfo<TT::P>,MultipleErrorInfo,TT::LW,TT::RW> {
    let nbpeer;
    let otherroute = if let MultipleReplyMode::OtherRoute = self.reply_mode {true} else{false};
    let revroute : Vec<TT::P>;
    let mut shad : Vec<TunnelShadowW<TT::P,MultipleReplyInfo<TT::P>,MultipleErrorInfo,TT::LW,TT::RW>>;
    { // restrict lifetime of peers for other route reply after
      let peers : Vec<&TT::P> = self.route_prov.new_route(p);
      nbpeer = peers.len();
      shad = Vec::with_capacity(nbpeer - 1);
      let mut errors = self.error_prov.new_error_route(&peers[..]);
      let mut replies = self.reply_prov.new_reply(&peers[..]);
      // TODO rem type
            let mut next_proxy_peer = None;
      let mut geniter = self.rng.gen_iter();

      for i in (1..nbpeer).rev() {
        shad.push(TunnelShadowW {
          shad : peers[i-1].get_shadower(true),
          next_proxy_peer : next_proxy_peer,
          tunnel_id : geniter.next().unwrap(),
          rep : replies.pop().unwrap(),
          err : errors.pop().unwrap(),
          replypayload : None,
        });
        next_proxy_peer = Some(peers[i-1].to_address());
      }


      assert!(shad.len() > 0);

      if shad.first().map_or(false,|s|

         s.rep.require_additional_payload() && !otherroute) {
           revroute = peers.iter().rev().map(|p|(*p).clone()).collect();
      } else {
        revroute = Vec::new();
      }
    }
    // shad is reversed for MultiW so checking dest is in first
    shad.first_mut().map(|s|
    // set reply payload write for dest
    if s.rep.require_additional_payload() {
      let old_mode = self.reply_mode.clone();
      self.reply_mode = MultipleReplyMode::RouteReply;
       // a bit redundant with require
       if otherroute {
         s.replypayload = Some((self.limiter_proto_w.clone(),self.tunrep.new_writer_with_route(&self.route_prov.new_reply_route(p))));
       } else {
         // messy TODO refactor new_writer to use iterator
         let rref : Vec<&TT::P> = revroute.iter().collect();
         // same route
         s.replypayload = Some((self.limiter_proto_w.clone(),self.tunrep.new_writer_with_route(&rref[..])));
       };
       self.reply_mode = old_mode;
    });

    CompExtW(MultiWExt::new(shad),self.limiter_proto_w.clone())

  }
  fn make_shads (&mut self, peers : &[&TT::P], state : TunnelState) -> Shadows<TT::P,MultipleReplyInfo<TT::P>,MultipleErrorInfo,TT::LW,TT::RW> {

    let nbpeer;
    let otherroute = if let MultipleReplyMode::OtherRoute = self.reply_mode {true} else{false};
    let revroute : Vec<TT::P>;
    let mut shad : Vec<TunnelShadowW<TT::P,MultipleReplyInfo<TT::P>,MultipleErrorInfo,TT::LW,TT::RW>>;
    { // restrict lifetime of peers for other route reply after
      nbpeer = peers.len();
      shad = Vec::with_capacity(nbpeer - 1);
      let mut errors = self.error_prov.new_error_route(&peers[..]);
      let mut replies = self.reply_prov.new_reply(&peers[..]);
      //  TODO rem type
            let mut next_proxy_peer = None;
      let mut geniter = self.rng.gen_iter();

      for i in (1..nbpeer).rev() {
        shad.push(TunnelShadowW {
          shad : peers[i-1].get_shadower(true),
          next_proxy_peer : next_proxy_peer,
          tunnel_id : geniter.next().unwrap(),
          rep : replies.pop().unwrap(),
          err : errors.pop().unwrap(),
          replypayload : None,
        });
        next_proxy_peer = Some(peers[i-1].to_address());
      }


      assert!(shad.len() > 0);

      if shad.first().map_or(false,|s|
         s.rep.require_additional_payload() && otherroute) {
           revroute = peers.iter().rev().map(|p|(*p).clone()).collect();
      } else {
        revroute = Vec::new();
      }
    }
    // shad is reversed for MultiW so checking dest is in first
    shad.first_mut().map(|s|
    // set reply payload write for dest
    if s.rep.require_additional_payload() {
       // a bit redundant with require
       if otherroute {
         s.replypayload = Some((self.limiter_proto_w.clone(),self.tunrep.new_writer_with_route(&self.route_prov.new_reply_route(peers[nbpeer -1]))));
       } else {
         // messy TODO refactor new_writer to use iterator
         let rref : Vec<&TT::P> = revroute.iter().collect();
         // same route
         s.replypayload = Some((self.limiter_proto_w.clone(),self.tunrep.new_writer_with_route(&rref[..])));
       };
    });

    CompExtW(MultiWExt::new(shad),self.limiter_proto_w.clone())

  }


  fn make_cache_id (&mut self, state : TunnelState) -> Option<Vec<u8>> {
    if state.do_cache() {
      Some(self.new_cache_id())
    } else {
      None
    }
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



impl<P : Peer, RI : RepInfo, EI : Info,LW : ExtWrite,TW : TunnelWriterExt> TunnelWriterExt for FullW<RI,EI,P,LW,TW> {
  fn write_dest_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
    if let TunnelState::ReplyOnce = self.state {

      // write all simkeys (inner dest_inof to open multi
      // not all key must be define
      let len : usize = self.shads.0.inner_extwrites().len();

      // TODO do not bin encode usize
      bin_encode(&len, w, SizeLimit::Infinite).map_err(|e|BincErr(e))?;
      for i in 0..len {
        match self.shads.0.inner_extwrites_mut().get_mut(i) {
          Some(ref mut sh) =>  {
            sh.err.write_read_info(w)?;
            sh.rep.write_read_info(w)?;
          },
          None => panic!("Error not writing sim dest k, will result in erronous read"),
        }
      }
    }
    Ok(())
  }


}
impl<P : Peer, RI : RepInfo, EI : Info,LW : ExtWrite,TW : TunnelWriterExt> ExtWrite for FullW<RI,EI,P,LW,TW> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    // write starte
    bin_encode(&self.state, w, SizeLimit::Infinite).map_err(|e|BincErr(e))?;
    // write connect info
    if let Some(cci) = self.current_cache_id.as_ref() {
      bin_encode(cci, w, SizeLimit::Infinite).map_err(|e|BincErr(e))?;
    }

    // write connect info 

    // write tunnel header
    self.shads.write_header(w)?;
/*
    // WARNING Big Buff here (only for emitter), with all ri in memory : to keep safe code
    // Only use to write ri and ei info from all shads encoded by all shads
    // TODO overhead could be lower with fix size buffer and redesign of write_dest_info to use
    // buffer instead of write
    // TODO not in read_header (only for dest), could consider reply info containing buffer(bad)
    // TODO might be removable (removal of useless traits)
    let mut buff = Cursor::new(Vec::new());
    self.write_dest_info(&mut buff)?;
    try!(self.write_all_into(w,buff.get_ref()));
    */
    Ok(())
  }


  #[inline]
  fn write_into<W : Write>(&mut self, w : &mut W, cont : &[u8]) -> Result<usize> {
    self.shads.write_into(w,cont)
  }

  #[inline]
  fn write_all_into<W : Write>(&mut self, w : &mut W, cont : &[u8]) -> Result<()> {
    self.shads.write_all_into(w,cont)
  }
  #[inline]
  fn flush_into<W : Write>(&mut self, w : &mut W) -> Result<()> {
    self.shads.flush_into(w)
  }

  #[inline]
  fn write_end<W : Write>(&mut self, w : &mut W) -> Result<()> {
    self.shads.write_end(w)
  }

}

impl<E : ExtRead, P : Peer, RI : RepInfo, EI : Info> FullR<RI,EI,P,E> {
  #[inline]
  pub fn as_read<'a,'b,R : Read>(&'b mut self, r : &'a mut R) -> CompR<'a, 'b, R, FullR<RI,EI,P,E>> {
    CompR::new(r,self)
  }

  /// TODO include in multi read function instead !!
#[inline]
  pub fn read_cacheid<R : Read> (r : &mut R) -> Result<Vec<u8>> {
    Ok(try!(bin_decode(r, SizeLimit::Infinite).map_err(|e|BindErr(e))))
  }



}

impl<E : ExtRead, P : Peer, RI : RepInfo, EI : Info> TunnelReaderNoRep for FullR<RI,EI,P,E> {



  fn is_dest(&self) -> Option<bool> {
    if let TunnelState::TunnelState = self.state {
      None
    } else {
      Some(self.next_proxy_peer.is_none())
    }
  }

}

impl<E : ExtRead, P : Peer, RI : RepInfo, EI : Info> TunnelReader for FullR<RI,EI,P,E> {
  type RI = RI;
  fn get_current_reply_info(&self) -> Option<&Self::RI> {
    self.current_reply_info.as_ref()
  }
}

impl<E : ExtRead, P : Peer, RI : RepInfo, EI : Info> TunnelReaderError for FullR<RI,EI,P,E> {
  type EI = EI;
  fn get_current_error_info(&self) -> Option<&Self::EI> {
    self.current_error_info.as_ref()
  }
}

impl<E : ExtRead, P : Peer, RI : RepInfo, EI : Info> ExtRead for FullR<RI,EI,P,E> {
  #[inline]
  fn read_header<R : Read>(&mut self, r : &mut R) -> Result<()> {



    // read_state
    self.state = bin_decode(r, SizeLimit::Infinite).map_err(|e|BindErr(e))?;

    // read_connect_info
    // reading for cached reader
    if self.state.do_cache() {
      self.current_cache_id = Some(bin_decode(r, SizeLimit::Infinite).map_err(|e|BindErr(e))?);
    }

    // read_tunnel_header
    if !self.state.from_cache() {
      try!(self.shad.read_header(r));

      let mut inr  = CompExtRInner(r, &mut self.shad);

      self.next_proxy_peer = bin_decode(&mut inr, SizeLimit::Infinite).map_err(|e|BindErr(e))?;
      self.tunnel_id = Some(bin_decode(&mut inr, SizeLimit::Infinite).map_err(|e|BindErr(e))?);
      self.current_error_info = Some(EI::read_from_header(&mut inr)?);
      let ri = RI::read_from_header(&mut inr)?;

      //try!(self.err.write_in_header(&mut inw));
      //try!(self.rep.write_in_header(&mut inw));

      /*    let mut inw  = CompExtWInner(w, &mut self.shad);
            try!(bin_encode(&self.next_proxy_peer, &mut inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));
            */

      if ri.require_additional_payload() {
        self.content_limiter.read_header(&mut inr)?;
      };
      self.current_reply_info = Some(ri);

    }

    // read_dest_info // TODO move into  destfull init (dest_read_keys to destfullkind::Multi!!)
    // call in new dest reader i think -> Require refacto where dest reader is init with ref to
    // reader (previous code could also be split in initialiser (state dependant)
    self.dest_read_keys = if let TunnelState::ReplyOnce = self.state {

      let len : usize = bin_decode(r, SizeLimit::Infinite).map_err(|e|BindErr(e))?;

      let mut res = Vec::with_capacity(len);
      for _ in 0..len {
        self.current_error_info.as_mut().unwrap().read_read_info(r)?;// should always be init.
        self.current_reply_info.as_mut().unwrap().read_read_info(r)?;// should always be init.
        // TODO replace unwrap by return 
        let k : &Vec<u8> = self.current_reply_info.as_ref().ok_or(IoError::new(IoErrorKind::Other, "unparsed reply info"))?
          .get_reply_key().as_ref().ok_or(IoError::new(IoErrorKind::Other, "no reply key for reply info : wrong reply info"))?;

        res.push (k.clone());
      }
      Some(res)
    } else {
      None
    };

    Ok(())


      // TODO plus additional limiter    self.current_error_info = 
      //    self.current_reply_info = 


      /*let mut rep_key : Option<<<P as Peer>::Shadow as Shadow>::ShadowSim> = None;
        match tun_mode {
        TunnelMode::NoTunnel => self.shadow.1 = Some(TunnelProxyInfo {
        next_proxy_peer : None,
        tunnel_id : 0,
        tunnel_id_failure : None,
        error_handle : ErrorHandlingInfo::NoHandling,
        }), // to be dest

        _ => {


        try!(self.shadow.read_header(r));

      // no read if qerror and not dest if mod is not Full
      let has_key = match tun_state {
      TunnelState::QError => {
      if self.mode.is_het() {
      true
      } else {
      false
      }
      },
      TunnelState::ReplyOnce => true,
      TunnelState::QueryCached | TunnelState::QueryOnce => {
      if let TunnelMode::Tunnel(..) = tun_mode {
      true
      } else {false}
      },
      _ => false,
      };
      // read in start of enc shadow reader (as a tunnel info in reader)
      rep_key = if has_key {
      let mut inw  = CompExtRInner(r, &mut self.shadow);
      Some(try!(<<<P as Peer>::Shadow as Shadow>::ShadowSim as ShadowSim>::init_from_shadow_simkey(&mut inw)))
      }
      else { None};

      if tun_mode.is_het() {
      if let Some(true) = self.is_dest() {
      // try to read shacont header
      { 
      let mut inw  = CompExtRInner(r, &mut self.shadow);
      try!(self.shacont.read_header(&mut inw)); }
      try!(self.shadow.read_end(r));
      try!(self.shanocont.read_header(r)); // header of stop for last only
      }
      };
      }
      };
      self.mode = tun_mode;
      self.state = tun_state;
      self.rep_key = rep_key;
      Ok(())*/
  }

  #[inline]
  fn read_from<R : Read>(&mut self, r : &mut R, buf: &mut [u8]) -> Result<usize> {
 
    if self.current_reply_info.as_ref().map(|ri|ri.require_additional_payload()).unwrap_or(false) {
      let mut inr  = CompExtRInner(r, &mut self.shad);
      self.content_limiter.read_from(&mut inr,buf)
    } else {
      self.shad.read_from(r,buf)
    }

    /*    if let TunnelMode::NoTunnel = self.mode {
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
    }*/
  }
  #[inline]
  fn read_end<R : Read>(&mut self, r : &mut R) -> Result<()> {
    if self.current_reply_info.as_ref().map(|ri|ri.require_additional_payload()).unwrap_or(false) {
      self.content_limiter.read_end(r)?;
    }

    self.shad.read_end(r)

      /*
      // check if some reply route
      if (self.shadow).1.as_ref().map_or(false,|tpi|if let ErrorHandlingInfo::ErrorRoute = tpi.error_handle {true} else {false}) {

      try!(self.lim.read_end(r));

      }
      Ok(())*/
  }

}

//////-------------

/// override shadow for tunnel writes (additional needed info)
/// First ExtWrite is bytes_wr to use for proxying content (get end of encoded stream).
/// next is possible shadow key for reply,
/// And last is possible shadowroute for reply or error
pub struct TunnelShadowW<P : Peer, RI : Info, EI : Info,LW : ExtWrite,TW : TunnelWriterExt> {
  shad : <P as Peer>::Shadow,
  next_proxy_peer : Option<<P as Peer>::Address>,
  tunnel_id : usize, // tunnelId change for every hop that is description tci TODO should only be for cached reply info or err pb same for both : TODO useless ? error code are currently in error info
  rep : RI,
  err : EI,
  replypayload : Option<(LW,TW)>,
}

//pub struct TunnelShadowW<E : ExtWrite, P : Peer> (pub <P as Peer>::Shadow, pub TunnelProxyInfo<P>, pub Option<Vec<u8>>, Option<(E,Box<TunnelWriterFull<E,P>>)>);
// TODO switch to TunnelWriter impl and use default extwrite impl
impl<P : Peer, RI : RepInfo, EI : Info,LW : ExtWrite,TW : TunnelWriterExt> ExtWrite for TunnelShadowW<P,RI,EI,LW,TW> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    // write basic tunnelinfo and content
    try!(self.shad.write_header(w));
    let mut inw  = CompExtWInner(w, &mut self.shad);
    bin_encode(&self.next_proxy_peer, &mut inw, SizeLimit::Infinite).map_err(|e|BincErr(e))?;
    bin_encode(&self.tunnel_id, &mut inw, SizeLimit::Infinite).map_err(|e|BincErr(e))?;
    self.err.write_in_header(&mut inw)?;
    self.rep.write_in_header(&mut inw)?;
    if self.rep.require_additional_payload() {
      if let Some((ref mut content_limiter,ref mut rr)) = self.replypayload {
        content_limiter.write_header(&mut inw)?;
      }
    }
    Ok(())
  }
  #[inline]
  fn write_into<W : Write>(&mut self, w : &mut W, cont : &[u8]) -> Result<usize> {
    // this enum is obviously to costy (like general enum : full should specialize for multiple
    // writers (currently trait allow only one : so we need two different full (but still read
    // could be single if on same channel)
    if self.rep.require_additional_payload() {
      let mut inw  = CompExtWInner(w, &mut self.shad);
      if let Some((ref mut content_limiter,_)) = self.replypayload {
        return content_limiter.write_into(&mut inw, cont);
      }
    }
    self.shad.write_into(w,cont)
  }   
  #[inline]
  fn flush_into<W : Write>(&mut self, w : &mut W) -> Result<()> {
    if self.rep.require_additional_payload() {
      let mut inw  = CompExtWInner(w, &mut self.shad);
      if let Some((ref mut content_limiter,_)) = self.replypayload {
        return content_limiter.flush_into(&mut inw);
      }
    }
    self.shad.flush_into(w)
  }
  #[inline]
  fn write_end<W : Write>(&mut self, w : &mut W) -> Result<()> {
    if self.rep.require_additional_payload() {
      let mut inw  = CompExtWInner(w, &mut self.shad);
      if let Some((ref mut content_limiter,ref mut rr)) = self.replypayload {
        content_limiter.write_end(&mut inw)?;

        // write header (simkey are include in headers)
        rr.write_header(&mut inw)?;
        // write simkeys to read as dest (Vec<Vec<u8>>)
        // currently same for error
        // put simkeys for reply TODO only if reply expected : move to full reply route ReplyInfo
        // impl -> Reply Info not need to be write into
        rr.write_dest_info(&mut inw)?;

        try!(rr.write_end(&mut inw));
        try!(rr.flush_into(&mut inw));
      }
    }
    self.shad.write_end(w)
/*
    bad design both should be befor write end, but involve another limiter over content -> error or writer with route after changes full behavior : route reply could not be in Info but in TunnelShadowW
      or else do a rep info and err info method toknow if content include after : then use another limiter ~ ok
    Still skip after of reader : test of both include after , if one read to end is called with explicit limiter : write_after method need redesing to have its limiter as parameter

    -- No the plan will be to keep writer in replyinfo (remove latter from error as useless), then do write_after by checking 
    -- reply writer like proxy is base on TR so it can read from it, it also check RI for need of end read but not to read (proxying)
    -- the write info should also be at tunnelShadowW -> remove from reply and error TODO remember new limiter needed for content
    -- first step put reply in tunnel shadow W and link wired write correctly
    -- second step rewrite read for new if (with SR as input)
    -- third correct error and multi

    pb : route provider is related to error ? not really

    // reply or error route TODO having it after had write end is odd (even if at this level we are
    // into another tunnelw shadow), related with read up to reply TODO this might only be ok for
    // error (dif between EI and RI in this case)
    // This is simply that we are added to the end but in various layers.
    // reply or error route TODO having it after had write end is odd (even if at this level we are

    Ok(())*/
  }
}

// --- reader


// ------------ cached sym
pub struct TunnelCachedWriterExt<SW : ExtWrite,E : ExtWrite> {
  shads: CompExtW<SW,E>,
  dest_cache_id: Vec<u8>,
// TODO ??  dest_address: Vec<u8>,
}

pub type TunnelCachedWriterExtClone<SW,E> = Rc<RefCell<TunnelCachedWriterExt<SW,E>>>;
impl<SW : ExtWrite,E : ExtWrite> TunnelCachedWriterExt<SW,E> {
  pub fn new (sim : SW, next_cache_id : Vec<u8>, limit : E) -> Self
  {
    TunnelCachedWriterExt {
      shads : CompExtW(sim, limit),
      dest_cache_id : next_cache_id,
    }
  }
}


impl<SW : ExtWrite,E : ExtWrite> ExtWrite for TunnelCachedWriterExt<SW,E> {

  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(bin_encode(&TunnelState::ReplyCached, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    try!(bin_encode(&self.dest_cache_id, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    self.shads.write_header(w)
  }
  #[inline]
  fn write_into<W : Write>(&mut self, w : &mut W, cont: &[u8]) -> Result<usize> {
    self.shads.write_into(w,cont)
  }
  #[inline]
  fn write_all_into<W : Write>(&mut self, w : &mut W, buf : &[u8]) -> Result<()> {
    self.shads.write_all_into(w, buf)
  }
  #[inline]
  fn flush_into<W : Write>(&mut self, w : &mut W) -> Result<()> {
    self.shads.flush_into(w)
  }
  #[inline]
  fn write_end<W : Write>(&mut self, w : &mut W) -> Result<()> {
    self.shads.write_end(w)
  }
}

impl<OR : ExtRead,SW : ExtWrite, E : ExtWrite,LR : ExtRead> TunnelWriterExt for ProxyFull<OR,SW,E,LR> {

  #[inline]
  fn write_dest_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
    Ok(())
  }
}
impl<OR : ExtRead,SW : ExtWrite, E : ExtWrite,LR : ExtRead> ExtWrite for ProxyFull<OR,SW,E,LR> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    // write state
    match self.kind {
      ProxyFullKind::ReplyCached(_,_) => 
        bin_encode(&TunnelState::ReplyCached, w, SizeLimit::Infinite).map_err(|e|BincErr(e))?,
      ProxyFullKind::QueryCached(_,_) =>
        bin_encode(&TunnelState::QueryCached, w, SizeLimit::Infinite).map_err(|e|BincErr(e))?,
      ProxyFullKind::QueryOnce(ref state,_) | ProxyFullKind::ReplyOnce(ref state,_,_,_,_,_) =>
        bin_encode(state, w, SizeLimit::Infinite).map_err(|e|BincErr(e))?,
    }
   
   
    // write connnect_info
    match self.kind {
      ProxyFullKind::ReplyCached(_,_) => (), // key is in internal header impl (in extwrite trait : write_connect_info does not make sense)
      ProxyFullKind::QueryOnce(_,_) => (),
      ProxyFullKind::ReplyOnce(_,_,_,_,_,_) => (),
      ProxyFullKind::QueryCached(ref k,_) =>
        bin_encode(k, w, SizeLimit::Infinite).map_err(|e|BincErr(e))?,
    }
   
    // write tunnel header
    match self.kind {
      ProxyFullKind::ReplyCached(ref mut inner,_) => inner.write_header(w)?,
      ProxyFullKind::ReplyOnce(_,_,_,_,ref mut lim,_) | ProxyFullKind::QueryOnce(_,ref mut lim) | ProxyFullKind::QueryCached(_,ref mut lim) => lim.write_header(w)?,
    }

    Ok(())
  }


  fn write_into<W : Write>(&mut self, w : &mut W, buf : &[u8]) -> Result<usize> {
    match self.kind {
      ProxyFullKind::ReplyCached(ref mut inner,_) => inner.write_into(w,buf),
      ProxyFullKind::QueryOnce(_,ref mut lim) | ProxyFullKind::QueryCached(_,ref mut lim) => lim.write_into(w,buf),

      // in header proxying and in header reading
      ProxyFullKind::ReplyOnce(_,_,true,_,ref mut lim,true) => {
        lim.write_into(w,buf)
      },
      // read in payload
      ProxyFullKind::ReplyOnce(_,_,false,ref mut payw,ref mut lim,ref mut b) => {
        // require to switch
        if *b {
          lim.write_end(w)?;
          payw.write_header(w)?;
          *b = false;
        }
        payw.write_into(w,buf)
      },
      // TODO use a single state to avoid this
      ProxyFullKind::ReplyOnce(_,_,true,_,_,false) => panic!("Inconsistent replyonce proxy state read in header and write in content"),

    }
  }

  fn write_all_into<W : Write>(&mut self, w : &mut W, buf : &[u8]) -> Result<()> {
    match self.kind {
      ProxyFullKind::ReplyCached(ref mut inner,_) => inner.write_all_into(w,buf),
      ProxyFullKind::QueryOnce(_, ref mut lim) | ProxyFullKind::QueryCached(_, ref mut lim) => lim.write_all_into(w,buf),
      // a bit suboptimal as default impl
      ProxyFullKind::ReplyOnce(..) => {
        let mut cdw = CompExtW(DefaultID(), self);
        cdw.write_all_into(w,buf)
      },
    }
  }

  /// ExtWrite flush into
  fn flush_into<W : Write>(&mut self, w : &mut W) -> Result<()> {
    match self.kind {
      ProxyFullKind::ReplyCached(ref mut inner,_) => inner.flush_into(w),
      ProxyFullKind::QueryOnce(_, ref mut lim) | ProxyFullKind::QueryCached(_, ref mut lim) => lim.flush_into(w),
      ProxyFullKind::ReplyOnce(_,_,_,_,ref mut lim,true) => {
        lim.flush_into(w)
      },
      ProxyFullKind::ReplyOnce(_,_,_,ref mut payw,_,false) => {
        payw.flush_into(w)
      },
    }
  }
  /// ExtWrite write end
  fn write_end<W : Write>(&mut self, w : &mut W) -> Result<()> {
    match self.kind {
      ProxyFullKind::ReplyCached(ref mut inner,_) => inner.write_end(w),
      ProxyFullKind::QueryOnce(_,ref mut lim) | ProxyFullKind::QueryCached(_, ref mut lim) => lim.write_end(w),
      ProxyFullKind::ReplyOnce(_,_,_,ref mut payw,ref mut lim,true) => {
        lim.write_end(w)?;
        payw.write_header(w)?;
        payw.write_end(w)
      },
      ProxyFullKind::ReplyOnce(_,_,_,ref mut payw,ref mut lim,ref mut b) => {
        *b = true;
        payw.write_end(w)
      },
    }
  }
}

/**
 * reader for a dest of tunnel cached
 */
pub type TunnelCachedReaderExt<SR,E> = CompExtR<MultiRExt<SR>,E>;

pub type TunnelCachedReaderExtClone<SR,E> =Rc<RefCell<TunnelCachedReaderExt<SR,E>>>;
fn new_dest_cached_reader_ext<SR : ExtRead, E : ExtRead> (sim : Vec<SR>, limit : E) -> TunnelCachedReaderExt<SR,E> {
  CompExtR(MultiRExt::new(sim), limit)
}


pub struct DestFull<OR : ExtRead,SR : ExtRead, E : ExtRead> {
  pub origin_read : OR,
  pub kind : DestFullKind<SR,E>
}
impl<OR : ExtRead,SR : ExtRead, E : ExtRead> TunnelReaderExt for DestFull<OR,SR,E> {
  type TR = OR; 
  fn get_reader(self) -> Self::TR {
    self.origin_read
  }
}

impl<OR : ExtRead,SR : ExtRead, E : ExtRead> ExtRead for DestFull<OR,SR,E> {
  fn read_header<R : Read>(&mut self, _ : &mut R) -> Result<()> {
    // actually already called
    Ok(())
  }

  fn read_from<R : Read>(&mut self, r : &mut R, buf : &mut[u8]) -> Result<usize> {
    match self.kind {
      DestFullKind::Multi(ref mut rs) => rs.read_from(r,buf),
      DestFullKind::Id => self.origin_read.read_from(r,buf),
    }
  }

  fn read_exact_from<R : Read>(&mut self, r : &mut R, mut buf: &mut[u8]) -> Result<()> {
    match self.kind {
      DestFullKind::Multi(ref mut rs) => rs.read_exact_from(r, buf),
      DestFullKind::Id => self.origin_read.read_exact_from(r,buf),
    }
  }

  fn read_end<R : Read>(&mut self, r : &mut R) -> Result<()> {
    match self.kind {
      DestFullKind::Multi(ref mut rs) => rs.read_end(r),
      DestFullKind::Id => self.origin_read.read_end(r),
    }
  }
}

/// kind of destreader
pub enum DestFullKind<SR : ExtRead, LR : ExtRead> {
  /// Multi : for reply
  Multi(TunnelCachedReaderExt<SR,LR>),
  /// Nothing directly ok
  Id,
}


