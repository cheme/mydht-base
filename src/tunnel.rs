//! tunnel : primitive function for tunnel (for both modes
//!
//! TODO reader and writer with possible two different size limiter for het mode (head limiter
//! should be small when content big or growable)
//!
//!
//!
//! # Frame schema :
//!
//! [] : content is limit by frame limiter, shadow plus end frame writer.
//! [ is seen as the header and ] as the end content of rwext.
//! [n ] : same with n as int note
//! () : content size is know, only shadow
//!
//! all content is either shadowed or binary encoded
//!
//! Description for PublicTunnel are skipped : it is the same as BiTunnel without reply (a reply to
//! a query is included in a PublicTunnel query) info
//! (possibly a mode where error behaved like cache Tunnel).
//!
//!
//! Error reply could be disable for each case, then the frame is the same without those infos.
//!
//! It is good to remember that no reply and no error reply ensure better security (less trace).
//! Yet reply info (error token, error token xor, and rep key) are only all known by emitter and
//! individually known by their designed peer.
//!
//! The route could be use one or more time, yet the mecanism for reused is not describe here (we
//! consider unique use). Some notes are added about reuse strategy. Mostly reuse is running in
//! reply like mode (sym shadow with known info, no need for headers). Reuse also involve specific
//! TunnelState
//!
//! The description are written for three hop, 0 is emitter, 1 and 2 are proxy and 3 is dest,
//! obviously more hop are easy to write (recursive). Error id of last is wrongly name as it is the
//! message id to access emmitter info (in Tunnel not bitunnel).
//!
//! tunnel mode in frame is both TunnelMode info (static) and TunnelState (corresponding to Query
//! or Reply obviously).
//!  
//!    tunnelmode := TunnelState TunnelMode
//!
//! Error code must be obviously secure random (with the exception that we may avoid route with xor
//! collision of error code).
//!
//! ## Tunnel
//!
//! tunnel does not include reply route, because the same route is used for query and reply.
//! It means that each peer need to maintain a cache (key val) of message to enable backward route.
//! Therefore content is smaller, but proxying is more cumbersome (in case where tunnel stay in
//! place for big content it could be better than BiTunnel (yet less secure)).
//!
//! Between each hop an unique tunnel msg cache id (*tci*) is use, it is the key to access info in cache,
//! such key are therefore known only by peer communicating (when proxy this key is added at the
//! end (previous value is not proxyied of course)).
//! tci must be obviously secure random.
//!
//! TODO tcn (cache key of each peer) should be send by 0 in headers and only transmit in reply or
//! optionaly in query if the key is already use in peer. -> do it latter as it 
//! TODO similarily when reuse, those ids should change (use old tci on all head but store a new
//! one for next usage (and reply)).
//!
//!
//! ### Last
//!
//! Last mode (true for BiTunnel and PublicTunnel to), is a mode used to be lighter for big
//! content, only the header is layered shadowed. Therefore this mode is way less secure than full
//! : *some bytes (the payload) even if shadowed are the same between tunnel hops*.
//!
//! 0 cache : the message key keysim (included only in dest head), the ordered error ids (eid), and
//!   the fact that we are destination for reply (no proxy)
//! n cache : previous tunnel cache id, previous peer id, errorcodeid
//! dest cache : nothing (except for reuse)
//!
//! #### query
//!
//!   TunnelMode is QueryOnce
//!
//!   tunnelmode tci0 [1 head1[1 head2[1 head3 [2 ]]][3 payload 2]3]
//!
//!   tunnelmode tci1 [1 head2[1 head3 [2 ]][3 payload 2]3]
//!
//!   tunnelmode tci2 [1 head3 [2 ][3 payload 2]3]
//!
//!    
//!   1 : shadow asym, we could use asym only
//!   2 : shadow asym, this implementation is for long content and should use sym shadow
//!     internally. The header is only for the shadow part (not the frame limitter)
//!   3 : frame limitter header, this one would be read and rewrite at each proxy hop
//!
//!   Notice that 2 and 3 compose to a standard shadow with frame limitter, except that header are
//!   reversed in order (the header of shadow is still limitted by the dest3 head limiter. This
//!   also implies that dest need to *read/write 3 header without applying 2* (after that juust read 3 over
//!   2).
//!
//!   
//!
//! Total content size could be bigger than in full mode (depending on shadow used and rwext frame
//! limiter), as there is more frame limitter and possible heteregenous shadow.
//! 
//! Proxy : tunnelmode is read, previouspeer ticket (tci) is read, Headn is read (we identify
//! ourself as proxy). Tci and reply TunnelMode added to
//! cache info for reply. Tunnelmode is written again as is, our cache key (tci) is written unshadowed.
//! Remaining stack of head is read (with unshadow) and proxied as is,
//! finally remaining payload is read/written as is.
//!
//! (content include next headers).
//!
//! Reuse : simple here as we already got cache, proxy cache will keep reply tci from and next query should
//! only have payload and dest tci (plus special TunnelMode state (QueryCached))
//!
//! #### reply
//!
//!   Like for error TunnelState is ReplyCached.
//!
//!   TunnelState tci2 [1 payload]
//!   TunnelState tci1 [1 payload]
//!   TunnelState tci0 [1 payload]
//!
//!   1 : keysim shadowed content by dest (keysim is in TunnelProxyInfo 3 : aka head3 in previous
//!     diagram)
//!
//!   Proxy : read TunnelState, read our cache tci, get TunnelMode from cache, get dest and dest tci from cache,
//!   write TunnelState, write dest tci, read/write payload as is (using frame limiter r/w).
//!
//! #### error
//!
//!   if error in 3 (3 being a proxy in this case): 
//!
//!   TunnelState tci2 errorcode3
//!   TunnelState tci1 (1 errorcode3)
//!   TunnelState tci0 (1 (1 errorcode3))
//!
//!   1 : xor with errorcode except for error emitter (direct error code)
//!
//!   Proxy : read tunnelmode, our tci, get dest tci from cache (and peer id), read error code, write tunnelmode,
//!   dest tci and xor with our error code.
//!
//!   Knowing it is error (and not reply) is from TunnelState (tunnelmode is both TunnelMode and
//!   TunnelState).
//!
//!   Origin (ourself or 0) will simply xor errorcode (n-1 xored), by its cached ordered error code value until it
//!   got a known value (he got all error code in cache), indicating the peer who could not contact
//!   its next peer. On such error the dht should try to check connection (or not to avoid leaking
//!   info) and update its peer table accordingly then send in a new route (start of route could be
//!   kept (or not to avoid leaking info (if reuse we can consider it is fine))).
//!   Notice that this is subject to rare collision (this could be avoid by checking it at route
//!   construct but it is seems like useless cost (xor construction is in reverse order and xor
//!   read is in order : so it is not linear)).
//!
//! ### Full
//!
//! Full mode is standard layered proxy where every hop got to shadow/unshadow the full content.
//! The emiter multishadow n time for n hop on query and unmultishadow n time on reception.
//!
//! Full is more costly to forward as all message must be unshadow, therefore shadow mode should
//! combine asym and sym scheme.
//!
//!
//! Cache are the same as for Last except that 
//! 0 cache : message key keysim are multiple and ordered (n),
//! n cache : a message key keysim is added (read from its header)
//! dest cache : nothing (except for reuse)
//!
//! #### query
//!
//!   TunnelMode is QueryOnce
//!
//!   tunnelmode tci0 [1 head1[1 head2[1 head3 payload ]]]
//!   tunnelmode tci1 [1 head2[1 head3 payload ]]
//!   tunnelmode tci2 [1 head3 payload ]
//!    
//!   1 : shadow asym, this implementation is for long content and should use sym shadow
//!     internally. Done with peer shadow public knowledge.
//!
//!   head contain n cache info (in TunnelProxyInfo) with the error id, and the keysim for reply.
//!   
//! Proxy : tunnelmode is read, previouspeer ticket (tci) is read, Headn is read (we identify that
//! we are not dest). tci is added to cache info for reply. tunnelmode for reply is also added to
//! cache.
//!
//! Tunnelmode and our cache key (tci) are written as is unshadowed.
//! , all remaining content is read with unshadow and write unshadow 
//! (remaining content does include next headers).
//!
//! Reuse : similar to last.
//! 
//! #### reply
//!
//!   TunnelState tci2 [2 (1 payload)]
//!   TunnelState tci1 [2 (1 (1 payload))]
//!   TunnelState tci1 [2 (1 (1 (1 payload)))]
//!
//!   1 : keysim shadowed (by dest  or shadowed content by dest without frame limitter
//!   2 : frame limitter only (no shadow)
//!
//!   Proxy : same as for Last, except that we shadow content with our shadower.
//!
//! #### error
//!
//!   Same as Last
//!
//!
//! ## BiTunnel
//!
//!
//! Reply route could differ from query route.
//!
//! No tci is seen, only unshadow content is tunnelmode.
//!
//! Note that BiTunnel could be used with reply route being same as query route, just to avoid
//! local cache. 
//! It still is better than Tunnel because tci could not be analysed (or tci
//! conflict). 
//! Yet for reuse a Tunnel cache need to be use with tci mecanism (same as Tunnel but
//! with more tci), and in this case BiTunnel with two identical route is useless.
//!
//!
//! ### Last
//!
//! Similar to Tunnel, but reply headers are added after content.
//!
//! No cache (except in case of reuse where we switch to Tunnel like (the tci will have to be
//! inserted similarily) (with additional cache in
//! reply route)).
//!
//! #### query
//!
//!   TunnelMode is QueryOnce
//!
//!   Same frame as for Tunnel with reply route *rr* added and no tci (when not reusable). For instance :
//!
//!   tunnelmode [1 head1[1 head2[1 head3 [2 ]]][3 payload 2] rr 3]
//!
//!   In this case :
//!   2 : contains a frame limitter header this time (to know where shadowroute start).
//!   3 : still a frame limitter only.
//!
//!   head 3 contain the mkey for reply 
//!
//!   Detail of rr is :
//!
//!   [1 rhead2[1 rhead1[1 rhead0]]]
//!
//!   Note that rhead0 also contains the mkey (no cache for 0)
//!
//! Proxy : Same as Tunnel Last but no cache of info or tci read.
//!
//! #### reply
//!
//!   Dest will read write remaining rr as is (its header contains first peer to reply to), it
//!   explain why rr is not before payload : we need to read payload to know if we reply and there
//!   fore rr does not need to be read (no memory use but stuck transport : as doing a reply could
//!   be long we may want still want to cache the rr).
//!
//!   tunnelmode rr [3 (4replypayload)]
//!   tunnelmode [1 rhead1[1 rhead0]][3 (4replypayload)]
//!   tunnelmode [1 rhead0][3 (4replypayload)]
//!
//!   3 : frame limitter only (used by each proxy)
//!   4 : keysim (mkey) encoded content (used only by dest)
//!
//!
//!   Proxy : read TunnelState, read our cache tci, get TunnelMode from cache, get dest and dest tci from cache,
//!   write TunnelState, write dest tci, read/write payload as is (using frame limiter r/w).
//!
//! #### error
//!    
//!    No error or insert of rr for every rhead like this (big cost on frame size) :
//!
//!    Query frame became :
//!
//!    tunnelmode [1 head1 rr1[1 head2 rr2[1 head3 [2 ]]][3 payload 2] rr 3]
//!
//!    rrn are defined like rr.
//!
//!    Note that reply route should be all of similar length with different peers in each route (n distinct route).
//!
//!    Error is close to Tunnel (multiple xor and replace payload) : see Tunnel for detail. At peer
//!    n: 
//!
//!    tunnelmode rrn (errorcode)
//!
//!    TODO a mode could run error with same route by using tci (needed for reuse)
//!
//!
//! ### Full
//!
//! Payload is included in layer shadow.
//!
//!
//! #### query
//!
//!
//!   tunnelmode [1 head1[1 head2[1 head3 [2 payload ] rr ]]]
//!
//!   In this case :
//!   1 : see Full Tunnel
//!   2 : is frame limitter only to get.
//!
//!   rr is same as for Last.
//!
//!   But all rhead contains a unique mkey. (for reuse head contain one to but and head3 (dest) contain
//!   all head mkey)
//!   rhead0 contains all those ordered mkey (no cache)
//!
//! Proxy : Same as Full Tunnel but without caching anything or tci reading.
//!
//! #### reply
//!
//!   Similar to Last
//!
//!   tunnelmode rr [3 (4replypayload)]
//!   tunnelmode [1 rhead1[1 rhead0]][3 (4 (4replypayload))]
//!   tunnelmode [1 rhead0][3 (4(4(4replypayload)))]
//!
//!   3 : frame limitter only (used by each proxy)
//!   4 : keysim (mkey) encoded content (used only by dest)
//!
//!   Proxy : same as for Last, except that we shadow content with each proxy mkey (rhead0
//!   containing all rhead key).
//!
//! #### error
//!
//! Same as Last (error multiple xor is already Full).
//!    
//!
//!
//!
//!  *Implementation Status* : PublicTunnel
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
  Tunnel(u8,TunnelShadowMode,ErrorHandlingMode),
  /// Tunnel with different path forward and backward.
  /// param is nb of hop.
  /// Error will use forward route like normal tunnel if boolean is true, or backward route (very
  /// likely to fail and result in query timeout) if boolean is false
  BiTunnel(u8,TunnelShadowMode,bool,ErrorHandlingMode),
 
  /// in this mode origin and destination know themselve (cf message mode), we simply reply with same nb hop. Back
  /// route is therefore from dest (no need to include back route info). 
  PublicTunnel(u8,TunnelShadowMode,ErrorHandlingMode),
}

#[derive(RustcDecodable,RustcEncodable,Debug,Clone,PartialEq,Eq)]
pub enum TunnelState {
  /// query with all info to establish route (and reply)
  QueryOnce,
  /// info are cached
  QueryCached,
  /// Reply with no additional info for reuse
  ReplyOnce,
  /// info are cached
  ReplyCached,
  /// Error are only for query, as error for reply could not be emit again (dest do not know origin
  /// of query (otherwhise use PublicTunnel and only send Query).
  QError,
//  Query(usize), // nb of query possible TODO for reuse 
//  Reply(usize), // nb of reply possible TODO for reuse
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

/// Mode to use for error handle
#[derive(RustcDecodable,RustcEncodable,Debug,Clone,PartialEq,Eq)]
pub enum ErrorHandlingMode {
  /// do not propagate errors
  NoHandling,
  /// send error to peer with desingned mode (case where we can know emitter for dest or other error
  /// handling scheme with possible rendezvous point), TunnelMode is optional and used only for
  /// case where the reply mode differs from the original tunnelmode
  KnownDest(Option<Box<TunnelMode>>),
  /// route for error is included, end of payload should be proxyied.
  ErrorRoute,
}


/// Error handle info include in frame, also use for reply
#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
pub enum ErrorHandlingInfo<P : Peer> {
  NoHandling,
  KnownDest(<P as KeyVal>::Key, Option<TunnelMode>),
  ErrorRoute(<P as Peer>::Address),
}

impl TunnelMode {
  /// return a vec of error handling info starting at first peer (route start at ourself) and
  /// ending at dest (dest is reply info)
  /// error_route if it differs from route, note that error route could be sized one only or more
  /// if variable length or variable route)
  pub fn errorhandling_mode<P : Peer>(&self, route : &[&P], error_route : Option<&[&P]>) -> Vec<ErrorHandlingInfo<P>> {
     let mut res = vec![ErrorHandlingInfo::NoHandling; route.len() - 1];
     let error_route_len = error_route.map(|er|er.len()).unwrap_or(0);
     match self {
      &TunnelMode::NoTunnel => (),
      &TunnelMode::Tunnel(_,_, ref b) => {
        panic!("TODO implement");
      },
      &TunnelMode::BiTunnel(_,_,_, ref b) => {
        panic!("TODO implement");
      },
      &TunnelMode::PublicTunnel(_,_, ref b) => {
        match b {
          &ErrorHandlingMode::NoHandling => (),
          &ErrorHandlingMode::KnownDest(ref ob) => {
            res[route.len() - 2] = ErrorHandlingInfo::KnownDest(route[0].get_key(), ob.as_ref().map(|b|(**b).clone()));
          },
          &ErrorHandlingMode::ErrorRoute => {
            for i in 0..res.len() {
            if error_route_len == 0 {
              // same route use for reply
              res[i] = ErrorHandlingInfo::ErrorRoute(route.get(i).unwrap().to_address());
            } else {
              let ix = if error_route_len > i {
                error_route_len - i
              } else {
                0
              };
              // use other route
              res[i] = ErrorHandlingInfo::ErrorRoute(error_route.as_ref().unwrap().get(ix).unwrap().to_address());
            };
            }
            // dest is known dest (public mode)
            res[route.len() - 1] = ErrorHandlingInfo::KnownDest(route[0].get_key(), None)
          },
        }
      },
     };
     res
  }

 
  pub fn tunnel_shadow_mode(&self) -> TunnelShadowMode {
     match self {
      &TunnelMode::NoTunnel => TunnelShadowMode::NoShadow,
      &TunnelMode::Tunnel(_,ref b,_) 
      | &TunnelMode::BiTunnel(_,ref b,_,_) 
      | &TunnelMode::PublicTunnel(_,ref b,_) => b.clone(),
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
/*
#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
/// QueryMode info to use in message between peers.
/// TODO delete : it is info that are read/write directly in methods (tunnelid)
pub enum TunnelModeMsg {
  Tunnel(u8,Option<usize>, TunnelID, TunnelShadowMode, ), //first u8 is size of tunnel for next hop of query, usize is size of descriptor for proxying info, if no usize it means it is fully shadowed and a pair is under it
  BiTunnel(u8,bool,Option<usize>,TunnelID,TunnelShadowMode), // see tunnel, plus if bool is true we error reply with forward route stored as laste param TODO replace Vec<u8> by Reader (need encodable of this reader as writalll
  PublicTunnel(u8,usize,TunnelID,TunnelShadowMode),
}
*/
#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
pub struct TunnelProxyInfo<P : Peer> {
  pub next_proxy_peer : Option<<P as Peer>::Address>,
  pub tunnel_id : usize, // tunnelId change for every hop
  pub tunnel_id_failure : Option<usize>, // if set that is a failure
  pub error_handle : ErrorHandlingInfo<P>, // error handle
}
/*
#[derive(RustcEncodable,Debug,Clone)]
pub struct TunnelProxyInfoSend<'a, P : Peer> {
  pub next_proxy_peer : Option<&'a <P as Peer>::Address>,
  pub tunnel_id : usize, // tunnelId change for every hop
  pub tunnel_id_failure : Option<usize>, // if set that is a failure
}*/

/*
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
}*/

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
  state: TunnelState,
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
  pub fn new(p : &P, e : E, mode : Option<TunnelMode>, state : Option<TunnelState>) -> TunnelReaderExt<E, P> {
    let mut s1 = p.get_shadower(false);
    let mut s2 = p.get_shadower(false);
    let m = mode.unwrap_or(TunnelMode::NoTunnel); // default to no tunnel
    let s = state.unwrap_or(TunnelState::QueryOnce); // default to query no cache

    TunnelReaderExt {
      shadow : TunnelShadowR(CompExtR(s1,e.clone()),None),
      shacont : s2,
      shanocont : e,
      mode : m,
      state : s,
    }
  }
}

impl<E : ExtRead, P : Peer> ExtRead for TunnelReaderExt<E, P> {
  #[inline]
  fn read_header<R : Read>(&mut self, r : &mut R) -> Result<()> {
    let tun_state = try!(bin_decode(r, SizeLimit::Infinite).map_err(|e|BindErr(e)));
    let tun_mode = try!(bin_decode(r, SizeLimit::Infinite).map_err(|e|BindErr(e)));
    match tun_mode {
       TunnelMode::NoTunnel => self.shadow.1 = Some(TunnelProxyInfo {
          next_proxy_peer : None,
          tunnel_id : 0,
          tunnel_id_failure : None,
          error_handle : ErrorHandlingInfo::NoHandling,
       }), // to be dest

       _ => {
        try!(self.shadow.read_header(r));
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



pub type TunnelWriter<'a, 'b, E : ExtWrite + 'b, P : Peer + 'b, W : 'a + Write> = CompW<'a,'b,W,TunnelWriterExt<E,P>>;
pub struct TunnelWriterExt<E : ExtWrite, P : Peer> {
  shads: CompExtW<MultiWExt<TunnelShadowW<P>>,E>,
  shacont: Option<CompExtW<<P as Peer>::Shadow,E>>, // shadow for content when heterogenous enc and last : the reader need to know the size of its content but E is external
  error: Option<usize>, // possible error hop to report
  mode: TunnelMode,
  state: TunnelState,
}
impl<E : ExtWrite + Clone, P : Peer> TunnelWriterExt<E, P> {
  #[inline]
  pub fn as_writer<'a,'b,W : Write>(&'b mut self, w : &'a mut W) -> TunnelWriter<'a, 'b, E, P, W> {
    CompW::new(w,self)
  }
  // new
  pub fn new(peers : &[&P], e : E, mode : TunnelMode, state : TunnelState, error : Option<usize>, 
    headmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
    contmode : <<P as Peer>::Shadow as Shadow>::ShadowMode,
    error_route : Option<&[&P]>,
  ) -> TunnelWriterExt<E, P> {
  
    let nbpeer = peers.len();
    let mut shad = Vec::with_capacity(nbpeer - 1);

    if let TunnelMode::NoTunnel = mode {
      // no shadow
    } else {
      let mut err_h = mode.errorhandling_mode(peers, error_route);
      let mut thrng = thread_rng();
      let mut geniter = thrng.gen_iter();
      let mut next_proxy_peer = None;

      let mut err_ix = 0;
      for i in  (1 .. peers.len()).rev() { // do not add first (is origin)
        let err = err_h.pop().unwrap(); // err_h is peers.len - 1, iter backward so pop ok
        let p = peers.get(i).unwrap();
        let tpi = TunnelProxyInfo {
          next_proxy_peer : next_proxy_peer,
          tunnel_id : geniter.next().unwrap(),
          tunnel_id_failure : None, // if set that is a failure
          error_handle : err,
        };
        next_proxy_peer = Some(p.to_address());
        let mut s = p.get_shadower(true);
        if mode.is_het() {
          s.set_mode(headmode.clone());
        } else {
          s.set_mode(contmode.clone());
        }
        shad.push(TunnelShadowW(s, tpi));
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
      shads : CompExtW(MultiWExt::new(shad),e.clone()),
      shacont : shacont,
      error: error,
      mode: mode,
      state: state,
    }
  }
}

impl<E : ExtWrite, P : Peer> ExtWrite for TunnelWriterExt<E, P> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(bin_encode(&self.state, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
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
pub struct TunnelShadowW<P : Peer> (pub <P as Peer>::Shadow, pub TunnelProxyInfo<P>);
//pub struct TunnelShadowW<E : ExtWrite, P : Peer> (pub CompExtW<E,<P as Peer>::Shadow>, pub TunnelProxyInfo<P>);
pub struct TunnelShadowR<E : ExtRead, P : Peer> (pub CompExtR<<P as Peer>::Shadow,E>, pub Option<TunnelProxyInfo<P>>);

impl<P : Peer> ExtWrite for TunnelShadowW<P> {
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
/*    println!("bef read tpi");
    let mut buf = vec![0;10];
    inr.read(&mut buf[..]);
    println!("debug : buf {:?}", &buf[..]);*/
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
  mut eproxy: EW,
  mut ew : EW,
  r : &mut R,
  w : &mut W) -> Result<()> {
    {
    try!(bin_encode(&tre.state, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    try!(bin_encode(&tre.mode, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    try!(eproxy.write_header(w));
    let mut sr = 1;
    while sr != 0 {
      sr = try!(tre.read_from(r, buf));
      let mut sw = 0;
      while sw < sr {
        let ssw = try!(eproxy.write_into(w,&buf[sw..sr - sw]));
        if ssw == 0 {
          return Err(IoError::new(IoErrorKind::Other, "Proxying failed, it seems we do not write all content"));
        }
        sw += ssw;
      }
    }
    try!(tre.read_end(r));
    }
    try!(eproxy.write_end(w));
    try!(eproxy.flush_into(w));
    // for het we only proxied the layered header : need to proxy the payload (end write of one
    // terminal shadow) : we read with end handling and write with same end handling
    if tre.mode.is_het() {
      try!(er.read_header(r));
      try!(ew.write_header(w));
      let mut sr = 1;
      while sr != 0 {
        sr = try!(er.read_from(r, buf));
        let mut sw = 0;
        while sw < sr {
          let ssw = try!(ew.write_into(w,&buf[sw..sr - sw]));
          if ssw == 0 {
            return Err(IoError::new(IoErrorKind::Other, "Proxying failed, it seems we do not write all content"));
          }
          sw += ssw;
        }
      }
      try!(er.read_end(r));
      try!(ew.write_end(w));
      try!(ew.flush_into(w));
    }

  Ok(())
}



// create a tunnel writter
// - list of peers (last one is dest)
// - mode : TunnelMode
// - the actual writer (transport most of the time as a mutable reference)


// test multiple tunnel (one hop) between two thread (similar to real world)

// all test in extra with rsa-peer : by mydht-base-test with RSaPeer


