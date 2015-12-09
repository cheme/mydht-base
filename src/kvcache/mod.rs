//! KVCache interface for implementation of storage with possible cache : Route, QueryStore,
//! KVStore.
//!
use std::collections::HashMap;
use std::marker::PhantomData;
use std::hash::Hash;
use mydhtresult::Result;
use rand::thread_rng;
use rand::Rng;
use bit_vec::BitVec;

pub mod rand_cache;
/// cache base trait to use in storage (transient or persistant) relative implementations
pub trait KVCache<K, V> : Sized {

  /// Add value, pair is boolean for do persistent local store, and option for do cache value for
  /// CachePolicy duration TODO ref to key (key are clone)
  fn add_val_c(& mut self, K, V);
  /// Get value TODO ret ref
  fn get_val_c<'a>(&'a self, &K) -> Option<&'a V>;
  fn has_val_c<'a>(&'a self, k : &K) -> bool {
    self.get_val_c(k).is_some()
  }
  /// update value, possibly inplace (depending upon impl (some might just get value modify it and
  /// set it again)), return true if update effective
  fn update_val_c<F>(& mut self, &K, f : F) -> Result<bool> where F : FnOnce(& mut V) -> Result<()>;
 
  /// Remove value
  fn remove_val_c(& mut self, &K) -> Option<V>;

  /// fold without closure over all content
  fn strict_fold_c<'a, B, F>(&'a self, init: B, f: F) -> B where F: Fn(B, (&'a K, &'a V)) -> B, K : 'a, V : 'a;
  /// very permissive fold (allowing closure)
  fn fold_c<'a, B, F>(&'a self, init: B, mut f: F) -> B where F: FnMut(B, (&'a K, &'a V)) -> B, K : 'a, V : 'a;

  // TODO lightweigh map (more lightweight than  fold (no fn mut), find name where result is a
  // Result () not a vec + default impl in trem of fold_c TODO a result for fail computation : work
  // in Mdht result
  /// map allowing closure, stop on first error, depending on sematic error may be used to end
  /// computation (fold with early return).
  fn map_inplace_c<'a,F>(&'a self, mut f: F) -> Result<()> where F: FnMut((&'a K, &'a V)) -> Result<()>, K : 'a, V : 'a {
    self.fold_c(Ok(()),|err,kv|{if err.is_ok() {f(kv)} else {err}})
  }

  fn len_c (& self) -> usize;
  fn len_where_c<F> (& self, f : &F) -> usize where F : Fn(&V) -> bool {
    self.fold_c(0, |nb, (_,v)|{
      if f(v) {
        nb + 1
      } else {
        nb
      }
    })
  }
//  fn it_next<'b>(&'b mut Self::I) -> Option<(&'b K,&'b V)>;
  //fn iter_c<'a, I>(&'a self) -> I where I : Debug;
  // TODO see if we can remove that
  fn new() -> Self;


  /// Return n random value from cache (should be used in routes using cache).
  ///
  /// Implementation should take care on returning less result if ratio of content in cache and
  /// resulting content does not allow a good random distribution (default implementation uses 0.66
  /// ratio).
  ///
  /// Warning default implementation is very inefficient (get size of cache and random depending
  /// on it on each values). Cache implementation should use a secondary cache with such values.
  /// Plus non optimized vec instantiation (buffer should be internal to cache).
  /// Note that without enough value in cache (need at least two time more value than expected nb
  /// result). And result is feed through a costy iteration.
  /// 
  fn next_random_values<'a,F>(&'a mut self, queried_nb : usize, f : Option<&F>) -> Vec<&'a V> where K : 'a,
  F : Fn(&V) -> bool,
  {
    
    let l = match f {
      Some(fil) => self.len_where_c(fil),
      None => self.len_c(),
    };
    let randrat = l * 2 / 3;
    let nb = if queried_nb > randrat {
      randrat
    } else {
      queried_nb
    };
    
    if nb == 0 {
      return Vec::new();
    }

    let xess = {
      let m = l % 8;
      if m == 0 {
        0
      } else {
        8 - m
      }
    };
    let mut v = if xess == 0 {
      vec![0; l / 8]
    } else {
      vec![0; (l / 8) + 1]
    };
    loop {
      thread_rng().fill_bytes(&mut v);
      match inner_cache_bv_rand(self,&v, xess, nb, f) {
        Some(res) => return res,
        None => {
     warn!("Rerolling random (not enough results)");
        },
      }
    };
  }
}

#[inline]
fn inner_cache_bv_rand<'a,K, V, C : KVCache<K,V>, F : Fn(&V) -> bool> (c : &'a C, buf : &[u8], xess : usize, nb : usize, filtfn : Option<&F>) -> Option<Vec<&'a V>>  where K : 'a {
 
  let filter = {
    let mut r = BitVec::from_bytes(buf);
    for i in 0..xess {
      r.set(i,false);
    }
    r
  };

  let fsize = filter.iter().filter(|x| *x).count();

  let ratio : usize = fsize / nb;
   if ratio == 0 {
     None
   } else {
     Some(c.strict_fold_c((xess,0,Vec::with_capacity(nb)), |(i, mut tot, mut r), (_,v)|{
       if filtfn.is_none() || (filtfn.unwrap())(v) {
         if filter[i] {
           if tot < nb && tot % ratio == 0 {
             r.push(v);
           }
           tot = tot + 1;
         }
         (i + 1, tot, r)
       } else {
         (i, tot, r)
       }
     }).2)
   }
}
 
pub struct NoCache<K,V>(PhantomData<(K,V)>);

impl<K,V> KVCache<K,V> for NoCache<K,V> {
  //type I = ();
  fn add_val_c(& mut self, _ : K, _ : V) {
    ()
  }
  fn get_val_c<'a>(&'a self, _ : &K) -> Option<&'a V> {
    None
  }
  fn update_val_c<F>(&mut self, _ : &K, _ : F) -> Result<bool> where F : FnOnce(&mut V) -> Result<()> {
    Ok(false)
  }
  fn remove_val_c(&mut self, _ : &K) -> Option<V> {
    None
  }
  fn strict_fold_c<'a, B, F>(&'a self, init: B, _: F) -> B where F: Fn(B, (&'a K, &'a V)) -> B, K : 'a, V : 'a {
    init
  }
  fn fold_c<'a, B, F>(&'a self, init: B, _ : F) -> B where F: FnMut(B, (&'a K, &'a V)) -> B, K : 'a, V : 'a {
    init
  }
  fn len_c (& self) -> usize {
    0
  }
/*  fn it_next<'b>(_ : &'b mut Self::I) -> Option<(&'b K,&'b V)> {
    None
  }*/
  fn new() -> Self {
    NoCache(PhantomData)
  }
}

impl<K: Hash + Eq, V> KVCache<K,V> for HashMap<K,V> {
  fn add_val_c(& mut self, key : K, val : V) {
    self.insert(key, val);
  }
  
  fn get_val_c<'a>(&'a self, key : &K) -> Option<&'a V> {
    self.get(key)
  }

  fn has_val_c<'a>(&'a self, key : &K) -> bool {
    self.contains_key(key)
  }

  fn update_val_c<F>(&mut self, k : &K, f : F) -> Result<bool> where F : FnOnce(&mut V) -> Result<()> {
    if let Some(x) = self.get_mut(k) {
      try!(f(x));
      Ok(true)
    } else {
      Ok(false)
    }
  }
  fn remove_val_c(& mut self, key : &K) -> Option<V> {
    self.remove(key)
  }

  fn new() -> Self {
    HashMap::new()
  }

  fn len_c (& self) -> usize {
    self.len()
  }

  fn strict_fold_c<'a, B, F>(&'a self, init: B, f: F) -> B where F: Fn(B, (&'a K, &'a V)) -> B, K : 'a, V : 'a {
    let mut res = init;
    for kv in self.iter(){
      res = f(res,kv);
    };
    res
  }
  fn fold_c<'a, B, F>(&'a self, init: B, mut f: F) -> B where F: FnMut(B, (&'a K, &'a V)) -> B, K : 'a, V : 'a  {
    let mut res = init;
    for kv in self.iter(){
      res = f(res,kv);
    };
    res
  }
  fn map_inplace_c<'a,F>(&'a self, mut f: F) -> Result<()> where F: FnMut((&'a K, &'a V)) -> Result<()> {
    for kv in self.iter(){
      try!(f(kv));
    };
    Ok(())
  }

}

#[test]
/// test of default random
fn test_rand_generic () {
  let t_none : Option <&(fn(&bool) -> bool)> = None;
  let filter_in = |b : &bool| *b;
  let filter = Some(&filter_in);
  let mut m : HashMap<usize, bool> = HashMap::new();
  assert!(0 == m.next_random_values(1,t_none).len());
  m.insert(1,true);
  assert!(0 == m.next_random_values(1,t_none).len());
  m.insert(2,true);
  assert!(1 == m.next_random_values(1,t_none).len());
  assert!(1 == m.next_random_values(2,t_none).len());
  m.insert(11,false);
  m.insert(12,false);
  assert!(2 == m.next_random_values(2,t_none).len());
  assert!(1 == m.next_random_values(2,filter).len());
  m.insert(3,true);
  assert!(1 == m.next_random_values(1,filter).len());
  assert!(2 == m.next_random_values(2,filter).len());
  // fill exactly 8 val
  for i in 4..9 {
    m.insert(i,true);
  }
  assert!(10 == m.len());
  assert!(1 == m.next_random_values(1,filter).len());
  assert!(5 == m.next_random_values(8,filter).len());
  m.insert(9,true);
  assert!(1 == m.next_random_values(1,filter).len());
  assert!(6 == m.next_random_values(8,filter).len());
}

