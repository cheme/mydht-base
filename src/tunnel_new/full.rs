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
use std::io::{
  Write,
  Read,
  Result,
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
  TunnelError,
  Info,
  RepInfo,
  BincErr,
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
  ReplyInfo,
};
use super::info::error::{
  MultiErrorInfo,
};
use super::nope::Nope;
use std::marker::PhantomData;
/// Generic Tunnel Traits, use as a traits container for a generic tunnel implementation
/// (to reduce number of trait parameter), it is mainly here to reduce number of visible trait
/// parameters in code
/// TODO reply info and error generic !!! after test for full ok (at least)
pub trait GenTunnelTraits {
  type P : Peer;
  type SSW : ExtWrite;
  type SSR : ExtRead;
  type TC : TunnelCache<Self::SSW,Self::SSR>;
  type SP : SymProvider<Self::SSW,Self::SSR>;
  /// Reply frame limiter (specific to use of reply once with header in frame
  /// TODO create a limiter trait which only is ExtWrite (to declare impl as limiter (in comp lib)?
  type RL : ExtWrite;
  /// Reply writer use only to include a reply envelope
  type RW : TunnelWriter;
  type RP : RouteProvider<Self::P>;
  type REP : ReplyProvider<Self::P, ReplyInfo<Self::RL,Self::P,Self::RW>>;
  type EP : ErrorProvider<Self::P, ReplyInfo<Self::RL,Self::P,Self::RW>>;
}

/// Reply and Error info for full are currently hardcoded MultipleReplyInfo
/// This is not the cas for FullW or FullR, that way only Full need to be reimplemented to use less 
/// generic mode TODO make it generic ? (associated InfoMode type and info constructor)
/// TODO remove E ??
pub struct Full<TT : GenTunnelTraits, E : ExtWrite> {
  pub me : TT::P,
  pub reply_mode : MultipleReplyMode,
  pub error_mode : MultipleReplyMode,
  pub cache : TT::TC,
  pub sym_prov : TT::SP,
  pub route_prov : TT::RP,
  pub reply_prov : TT::REP,
  pub error_prov : TT::EP,
  pub _p : PhantomData<(TT,E)>,
}

type Shadows<P : Peer, E : ExtWrite, RI : Info, EI : Info> = CompExtW<MultiWExt<TunnelShadowW<P,RI,EI>>,E>;

/**
 * No impl for instance when no error or no reply
 *
 *
 * Full writer : use for all write and reply, could be split but for now I use old code.
 *
 */
pub struct FullW<RI : RepInfo, EI : Info, P : Peer, E : ExtWrite > {
  state: TunnelState,
  /// Warning if set it means cache is use, it is incompatible with a non cache tunnel state
  /// (reading will fail if state is not right, so beware when creating FullW)
  current_cache_id: Option<Vec<u8>>,
  shads: Shadows<P,E,RI,EI>,
}

pub struct FullR {
}

pub struct FullSRW {
}

//impl TunnelNoRep for Full {

impl<TT : GenTunnelTraits, E : ExtWrite> TunnelNoRep for Full<TT,E> {
  type P = TT::P;
  // TODO a true ErrorInfo struct to remove error code from multiplereplyinfo
  type TW = FullW<ReplyInfo<TT::RL,TT::P,TT::RW>, MultiErrorInfo<TT::RL,TT::RW>, TT::P, E>;
  type TR = Nope; // TODO

  fn new_reader_no_reply (&mut self, _ : &Self::P) -> Self::TR {
    Nope()
  }
  fn new_writer_no_reply (&mut self, p : &Self::P) -> Self::TW {
    let state = TunnelState::ReplyOnce;
    let ccid = self.make_cache_id(state.clone());
    let shads = self.next_shads(p, state.clone());
    FullW {
      current_cache_id : ccid,
      state : state,
      shads: shads,
    }
  }
  fn new_writer_no_reply_with_route (&mut self, route : &[&Self::P]) -> Self::TW {
  let state = TunnelState::ReplyOnce;
    let ccid = self.make_cache_id(state.clone());
    let shads = self.make_shads(route, state.clone());
    FullW {
      current_cache_id : ccid,
      state : state,
      shads: shads,
    }

  }

}
impl<TT : GenTunnelTraits, E : ExtWrite> Tunnel for Full<TT,E> {
  // reply info info needed to established conn
  type RI = ReplyInfo<TT::RL,TT::P,TT::RW>;
  /// no info for a reply on a reply (otherwhise establishing sym tunnel seems better)
  type RTW = FullW<Nope, ReplyInfo<TT::RL,TT::P,TT::RW>, TT::P, E>;
  fn new_writer (&mut self, p : &Self::P) -> Self::TW {
    let state = TunnelState::QueryOnce;
    let ccid = self.make_cache_id(state.clone());
    let shads = self.next_shads(p, state.clone());
    FullW {
      current_cache_id : ccid,
      state : state,
      shads: shads,
    }
  }
  fn new_reply_writer (&mut self, p : &Self::P, ri : &Self::RI) -> Self::RTW {
    // TODO fn reply as an extwriter with ri as param may need to change RTW type
    unimplemented!()
  }
}


impl<TT : GenTunnelTraits, E : ExtWrite> TunnelError for Full<TT,E> {
  // TODO use a lighter error info type!!!
  type EI = ReplyInfo<TT::RL,TT::P,TT::RW>;
  /// can error on error, cannot reply also, if we need to reply or error on error that is a Reply 
  /// TODO make a specific type for error reply : either cached or replyinfo for full error
  type ETW = Nope;
  fn new_error_writer (&mut self, p : &Self::P, ei : &Self::EI) -> Self::ETW {
    
    if let MultipleReplyMode::CachedRoute = self.error_mode {
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
impl<TT : GenTunnelTraits, E : ExtWrite> TunnelManager for Full<TT,E> {

  // Shadow Sym (if established con)
  type SSW = TT::SSW;
  // Shadow Sym (if established con)
  type SSR = TT::SSR;

  fn put_symw(&mut self, w : Self::SSW) -> Result<Vec<u8>> {
    self.cache.put_symw_tunnel(w)
  }

  fn get_symw(&mut self, k : &[u8]) -> Result<&mut Self::SSW> {
    self.cache.get_symw_tunnel(k)
  }
  fn put_symr(&mut self, w : Self::SSR) -> Result<Vec<u8>> {
    self.cache.put_symr_tunnel(w)
  }

  fn get_symr(&mut self, k : &[u8]) -> Result<&mut Self::SSR> {
    self.cache.get_symr_tunnel(k)
  }

  fn use_sym_exchange (ri : &Self::RI) -> bool {
    ri.do_cache()
  }

  fn new_sym_writer (&mut self, k : Vec<u8>) -> Self::SSW {
    self.sym_prov.new_sym_writer(k)
  }

  fn new_sym_reader (&mut self, k : Vec<u8>) -> Self::SSR {
    self.sym_prov.new_sym_reader(k)
  }

  fn new_cache_id (&mut self) -> Vec<u8> {
    self.cache.new_cache_id()
  }

}


// This should be split for reuse in last or others (base fn todo)
impl<TT : GenTunnelTraits, E : ExtWrite> Full<TT,E> {

  // TODO fuse with make_shads (lifetime issue on full : need to open it but first finish
  // make_shads fn
  fn next_shads (&mut self, p : &TT::P, state : TunnelState) -> Shadows<TT::P,E,ReplyInfo<TT::RL,TT::P,TT::RW>,MultiErrorInfo<TT::RL,TT::RW>> {
    let peers : Vec<&TT::P> = self.route_prov.new_route(p);
    let nbpeer = peers.len();
    // TODO rem type
    let mut shad : Vec<TunnelShadowW<TT::P,ReplyInfo<TT::RL,TT::P,TT::RW>,MultiErrorInfo<TT::RL,TT::RW>>> = Vec::with_capacity(nbpeer - 1);
    // TODO rem type
    // Warning here some case were removed from original impl
    let mut add_symshad : Option<Vec<u8>> = if state.add_sim_key() {
         Some(Vec::new())
    }  else {None};
 

    panic!("TODO")
  }
  fn make_shads (&mut self, peers : &[&TT::P], state : TunnelState) -> Shadows<TT::P,E,ReplyInfo<TT::RL,TT::P,TT::RW>,MultiErrorInfo<TT::RL,TT::RW>> {
    let errors = self.error_prov.new_error_route(peers);
    let nbpeer = peers.len();
    // TODO rem type
    let mut shad : Vec<TunnelShadowW<TT::P,ReplyInfo<TT::RL,TT::P,TT::RW>,MultiErrorInfo<TT::RL,TT::RW>>> = Vec::with_capacity(nbpeer - 1);
    // TODO rem type
    // Warning here some case were removed from original impl
    let mut add_symshad : Option<Vec<u8>> = if state.add_sim_key() {
         Some(Vec::new())
    }  else {None};
 
/*  pub fn new<ER : ExtRead> (peers : &[(usize,&P)], e : E, mode : TunnelMode, state : TunnelState, error : Option<usize>, 
    headmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
    contmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
    error_route : Option<&[(usize,&P)]>,
    reply_route : Option<&[(usize,&P)]>, // for bitunnel only
    current_cache_id : Vec<u8>, // TODO optional (currently if not use empty vec)
    reader_limit : Option<ER>,
  ) -> Result<(TunnelWriterExt<E, P>, Option<TunnelCachedReaderExt<ER,P>>)> {
 
      let mut err_h = mode.errhandling_infos(peers, error_route);
      let mut thrng = thread_rng();
      let mut geniter = thrng.gen_iter();
      let mut next_proxy_peer = None;

      let mut err_ix = 0;
      for i in (1 .. peers.len()).rev() { // do not add first (is origin)
        let err = err_h.pop().unwrap_or(ErrorHandlingInfo::NoHandling); // err_h is peers.len - 1, iter backward so pop ok
        let p = peers.get(i).unwrap();
        let tpi = TunnelProxyInfo {
          next_proxy_peer : next_proxy_peer,
          tunnel_id : geniter.next().unwrap(),
          tunnel_id_failure : None, // if set that is a failure
          error_handle : err,
        };
        next_proxy_peer = Some(p.1.to_address());
        let mut s = p.1.get_shadower(true);
        if mode.is_het() {
          s.set_mode(headmode.clone());
        } else {
          s.set_mode(contmode.clone());
        }
        let rep_route = if i == 0  {match mode {
        TunnelMode::NoRepTunnel(..) => None,
        TunnelMode::Tunnel(..) => None,
        TunnelMode::BiTunnel(_,_,sameroute,_) => {
          let typed_none_read : Option<ER> = None;
          let w : TunnelWriterExt<E,P> = if sameroute {
            try!(Self::new(
              peers,  // TODO need reverse 
              e.clone(),
              TunnelMode::NoRepTunnel(0,TunnelShadowMode::Full,ErrorHandlingMode::NoHandling),  // No reply in norep tunnel
              TunnelState::ReplyOnce,
              None,
              headmode.clone(),
              contmode.clone(),
              None,
              None,
              Vec::new(), // no tcid for error or reply in different route
              typed_none_read,
            )).0
          } else {
             try!(Self::new(
              reply_route.as_ref().unwrap(), // TODO need reverse 
              e.clone(),
              TunnelMode::NoRepTunnel(0,TunnelShadowMode::Full,ErrorHandlingMode::NoHandling),  // No reply in norep tunnel
              TunnelState::ReplyOnce,
              None,
              headmode.clone(),
              contmode.clone(),
              None,
              None,
              Vec::new(), // no tcid for error or reply in different route
              typed_none_read,
            )).0
          };
          Some((e.clone(),Box::new(w)))
        },
        _ => panic!("unimplemented"),
      }} else {
        // TODO some optim on fn call out of loop
      if mode.insert_error_route() {
        let error_route_len = error_route.map(|er|er.len()).unwrap_or(0);
        // increasing length
          let typed_none_read : Option<ER> = None;
          let w  = if error_route_len == 0 {
            // same route use for reply
            try!(Self::new(
              &peers[0..i],  // TODO need reverse 
              e.clone(),
              TunnelMode::NoRepTunnel(0,TunnelShadowMode::Full,ErrorHandlingMode::NoHandling),  // No reply in norep tunnel
              TunnelState::QError,
              None,
              headmode.clone(),
              contmode.clone(),
              None,
              None,
              Vec::new(), // no tcid for error or reply in different route
              typed_none_read,
            )).0
          } else {
            let rlen = if i > error_route_len - 1 {
              error_route_len - 1
            } else {
              i
            };
            try!(Self::new(
              &error_route.as_ref().unwrap()[0..rlen], // TODO need reverse 
              e.clone(),
              TunnelMode::NoRepTunnel(0,TunnelShadowMode::Full,ErrorHandlingMode::NoHandling), 
              TunnelState::QError,
              None,
              headmode.clone(),
              contmode.clone(),
              None,
              None,
              Vec::new(), // no tcid for error or reply in different route
              typed_none_read,
            )).0
          };
          Some((e.clone(),Box::new(w)))
      } else {None}};


        match add_symshad.as_mut() {
          Some(ref mut v) => {
            let shadsim = try!(<<P as Peer>::Shadow as Shadow>::new_shadow_sim());
            // TODO interface where key returned as Vec<u8> (with those 3 lines as default impl)
            let mut buf = Cursor::new(Vec::new());
            try!(shadsim.send_shadow_simkey(&mut buf)); 
            let ibuf = buf.into_inner();
//            println!("key {:?}",ibuf);
            shad.push(TunnelShadowW(s, tpi,Some(ibuf),rep_route));
            v.push(shadsim);
          },
          None => {
            shad.push(TunnelShadowW(s, tpi,None,rep_route));
          },
        }
      }
    }
    let shacont = if mode.is_het() {
      peers.last().map(|p|{
        let mut s = p.1.get_shadower(true);
        s.set_mode(contmode.clone());
        CompExtW(s,e.clone())
      })
    } else {
      None
    };


    // reader
    let or = add_symshad.map(|mut v|{
      //v.reverse();
      new_dest_cached_reader_ext::<ER, P>(v,reader_limit.unwrap())
    });

    Ok((TunnelWriterExt {
      shads : CompExtW(MultiWExt::new(shad),e.clone()),
      shacont : shacont,
      error: error,
      mode: mode,
      state: state,
      current_cache_id : current_cache_id,
    }, or))
  }
*/

    unimplemented!()
  }

  fn make_cache_id (&mut self, state : TunnelState) -> Option<Vec<u8>> {
    if state.do_cache() {
      Some(self.new_cache_id())
    } else {
      None
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


/*  pub fn new<ER : ExtRead> (peers : &[(usize,&P)], e : E, mode : TunnelMode, state : TunnelState, error : Option<usize>, 
    headmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
    contmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
    error_route : Option<&[(usize,&P)]>,
    reply_route : Option<&[(usize,&P)]>, // for bitunnel only
    current_cache_id : Vec<u8>, // TODO optional (currently if not use empty vec)
    reader_limit : Option<ER>,
  ) -> Result<(TunnelWriterExt<E, P>, Option<TunnelCachedReaderExt<ER,P>>)> {
    let nbpeer = peers.len();
    let mut shad = Vec::with_capacity(nbpeer - 1);
    let mut add_symshad = if let TunnelMode::Tunnel(..) = mode {
      match state {
        TunnelState::QueryCached | TunnelState::QueryOnce => Some(Vec::new()),
        _ => None,
      }
    } else if let TunnelMode::BiTunnel(..) = mode {
      match state {
        // reply keys for enc in reply header
        TunnelState::ReplyOnce => Some(Vec::new()),
        _ => None,
      }
    } else {None};
 
    if let TunnelMode::NoTunnel = mode {
      // no shadow
    } else {
      let mut err_h = mode.errhandling_infos(peers, error_route);
      let mut thrng = thread_rng();
      let mut geniter = thrng.gen_iter();
      let mut next_proxy_peer = None;

      let mut err_ix = 0;
      for i in (1 .. peers.len()).rev() { // do not add first (is origin)
        let err = err_h.pop().unwrap_or(ErrorHandlingInfo::NoHandling); // err_h is peers.len - 1, iter backward so pop ok
        let p = peers.get(i).unwrap();
        let tpi = TunnelProxyInfo {
          next_proxy_peer : next_proxy_peer,
          tunnel_id : geniter.next().unwrap(),
          tunnel_id_failure : None, // if set that is a failure
          error_handle : err,
        };
        next_proxy_peer = Some(p.1.to_address());
        let mut s = p.1.get_shadower(true);
        if mode.is_het() {
          s.set_mode(headmode.clone());
        } else {
          s.set_mode(contmode.clone());
        }
        let rep_route = if i == 0  {match mode {
        TunnelMode::NoRepTunnel(..) => None,
        TunnelMode::Tunnel(..) => None,
        TunnelMode::BiTunnel(_,_,sameroute,_) => {
          let typed_none_read : Option<ER> = None;
          let w : TunnelWriterExt<E,P> = if sameroute {
            try!(Self::new(
              peers,  // TODO need reverse 
              e.clone(),
              TunnelMode::NoRepTunnel(0,TunnelShadowMode::Full,ErrorHandlingMode::NoHandling),  // No reply in norep tunnel
              TunnelState::ReplyOnce,
              None,
              headmode.clone(),
              contmode.clone(),
              None,
              None,
              Vec::new(), // no tcid for error or reply in different route
              typed_none_read,
            )).0
          } else {
             try!(Self::new(
              reply_route.as_ref().unwrap(), // TODO need reverse 
              e.clone(),
              TunnelMode::NoRepTunnel(0,TunnelShadowMode::Full,ErrorHandlingMode::NoHandling),  // No reply in norep tunnel
              TunnelState::ReplyOnce,
              None,
              headmode.clone(),
              contmode.clone(),
              None,
              None,
              Vec::new(), // no tcid for error or reply in different route
              typed_none_read,
            )).0
          };
          Some((e.clone(),Box::new(w)))
        },
        _ => panic!("unimplemented"),
      }} else {
        // TODO some optim on fn call out of loop
      if mode.insert_error_route() {
        let error_route_len = error_route.map(|er|er.len()).unwrap_or(0);
        // increasing length
          let typed_none_read : Option<ER> = None;
          let w  = if error_route_len == 0 {
            // same route use for reply
            try!(Self::new(
              &peers[0..i],  // TODO need reverse 
              e.clone(),
              TunnelMode::NoRepTunnel(0,TunnelShadowMode::Full,ErrorHandlingMode::NoHandling),  // No reply in norep tunnel
              TunnelState::QError,
              None,
              headmode.clone(),
              contmode.clone(),
              None,
              None,
              Vec::new(), // no tcid for error or reply in different route
              typed_none_read,
            )).0
          } else {
            let rlen = if i > error_route_len - 1 {
              error_route_len - 1
            } else {
              i
            };
            try!(Self::new(
              &error_route.as_ref().unwrap()[0..rlen], // TODO need reverse 
              e.clone(),
              TunnelMode::NoRepTunnel(0,TunnelShadowMode::Full,ErrorHandlingMode::NoHandling), 
              TunnelState::QError,
              None,
              headmode.clone(),
              contmode.clone(),
              None,
              None,
              Vec::new(), // no tcid for error or reply in different route
              typed_none_read,
            )).0
          };
          Some((e.clone(),Box::new(w)))
      } else {None}};


        match add_symshad.as_mut() {
          Some(ref mut v) => {
            let shadsim = try!(<<P as Peer>::Shadow as Shadow>::new_shadow_sim());
            // TODO interface where key returned as Vec<u8> (with those 3 lines as default impl)
            let mut buf = Cursor::new(Vec::new());
            try!(shadsim.send_shadow_simkey(&mut buf)); 
            let ibuf = buf.into_inner();
//            println!("key {:?}",ibuf);
            shad.push(TunnelShadowW(s, tpi,Some(ibuf),rep_route));
            v.push(shadsim);
          },
          None => {
            shad.push(TunnelShadowW(s, tpi,None,rep_route));
          },
        }
      }
    }
    let shacont = if mode.is_het() {
      peers.last().map(|p|{
        let mut s = p.1.get_shadower(true);
        s.set_mode(contmode.clone());
        CompExtW(s,e.clone())
      })
    } else {
      None
    };


    // reader
    let or = add_symshad.map(|mut v|{
      //v.reverse();
      new_dest_cached_reader_ext::<ER, P>(v,reader_limit.unwrap())
    });

    Ok((TunnelWriterExt {
      shads : CompExtW(MultiWExt::new(shad),e.clone()),
      shacont : shacont,
      error: error,
      mode: mode,
      state: state,
      current_cache_id : current_cache_id,
    }, or))
  }
*/
}

/// Wrapper over TunnelWriter to aleviate trait usage restrictions, WriterExt is therefore to be
/// implemented on this (see full.rs)
/// This could be removed after specialization TODO non private field after redesign deps in
/// multi.rs
pub struct TunnelWriterFull<TW : TunnelWriter> (pub TW);

/// TODO this impl must move to all TunnelWriter
impl<TW : TunnelWriter> ExtWrite for TunnelWriterFull<TW> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(self.0.write_state(w));
    try!(self.0.write_connect_info(w));
    try!(self.0.write_tunnel_header(w));
    Ok(())
  }

  #[inline]
  fn write_into<W : Write>(&mut self, w : &mut W, cont : &[u8]) -> Result<usize> {
    self.0.write_tunnel_into(w,cont)
  }

  #[inline]
  fn write_all_into<W : Write>(&mut self, w : &mut W, cont : &[u8]) -> Result<()> {
    self.0.write_tunnel_all_into(w,cont)
  }

  #[inline]
  fn flush_into<W : Write>(&mut self, w : &mut W) -> Result<()> {
    self.0.flush_tunnel_into(w)
  /*  if let TunnelMode::NoTunnel = self.mode {
      Ok(())
    } else {
    match self.shacont.as_mut() {
      Some (s) => s.flush_into(w),
      None => self.shads.flush_into(w),
    }
    }*/
  }

  #[inline]
  fn write_end<W : Write>(&mut self, w : &mut W) -> Result<()> {
    self.0.write_tunnel_end(w)
/*    if let TunnelMode::NoTunnel = self.mode {
      return Ok(())
    } else {
      match self.shacont.as_mut() {
        Some (s) => try!(s.write_end(w)),
        None => try!(self.shads.write_end(w)),
      }
    }
    Ok(())
*/
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


impl<E : ExtWrite, P : Peer, RI : RepInfo, EI : Info> TunnelWriter for FullW<RI,EI,P,E> {
  #[inline]
  fn write_state<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(bin_encode(&self.state, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    Ok(())
  }

  #[inline]
  fn write_connect_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
    // redundant first test could be usefull if FullW wrongly created
//    if self.state.do_cache() {
    if let Some(cci) = self.current_cache_id.as_ref() {
      try!(bin_encode(cci, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    }
//    }
    Ok(())
  }
  #[inline]
  fn write_tunnel_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(self.shads.write_header(w));
    /* shacont is for last : TODO get it then
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
  fn write_tunnel_into<W : Write>(&mut self, w : &mut W, cont : &[u8]) -> Result<usize> {
    // for lasst : 
//    match self.shacont.as_mut() {
 //     Some (s) => s.write_into(w,cont),
    self.shads.write_into(w,cont)
  }

  #[inline]
  fn write_tunnel_all_into<W : Write>(&mut self, w : &mut W, cont : &[u8]) -> Result<()> {
    self.shads.write_all_into(w,cont)
  }

  #[inline]
  fn flush_tunnel_into<W : Write>(&mut self, w : &mut W) -> Result<()> {
  // for last
//    match self.shacont.as_mut() {
 //     Some (s) => s.flush_into(w),
    self.shads.flush_into(w)
  }

  #[inline]
  fn write_tunnel_end<W : Write>(&mut self, w : &mut W) -> Result<()> {
  // for last
      //match self.shacont.as_mut() {
       // Some (s) => try!(s.write_end(w)),
    self.shads.write_end(w)
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
pub struct TunnelShadowW<P : Peer, RI : Info, EI : Info> {

  shad : <P as Peer>::Shadow,
  next_proxy_peer : Option<<P as Peer>::Address>,
  tunnel_id : usize, // tunnelId change for every hop that is description tci TODO should only be for cached reply info or err pb same for both
  rep : RI,
  err : EI,

}

//pub struct TunnelShadowW<E : ExtWrite, P : Peer> (pub <P as Peer>::Shadow, pub TunnelProxyInfo<P>, pub Option<Vec<u8>>, Option<(E,Box<TunnelWriterFull<E,P>>)>);

impl<P : Peer, RI : Info, EI : Info> ExtWrite for TunnelShadowW<P,RI,EI> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    // write basic tunnelinfo and content
    try!(self.shad.write_header(w));
    let mut inw  = CompExtWInner(w, &mut self.shad);
    try!(bin_encode(&self.next_proxy_peer, &mut inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    try!(bin_encode(&self.tunnel_id, &mut inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    try!(self.err.write_in_header(&mut inw));
    try!(self.rep.write_in_header(&mut inw));
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
    try!(self.err.write_after(w));
    try!(self.rep.write_after(w));

    Ok(())
  }
}



