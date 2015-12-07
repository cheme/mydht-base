use super::{
  KVCache,
  inner_cache_bv_rand,
};
use mydhtresult::Result;
use std::marker::PhantomData;
use rand::thread_rng;
use rand::Rng;

#[cfg(test)]
use std::collections::HashMap;

/// automatic implementation of random for cache by composition
pub struct RandCache<K, V, C : KVCache<K,V>> {
  cache : C,

  // cache to limit number of call to rand
  randcache : Vec<u8>,

  randcachepos : usize,
  numratio : usize,
  denratio : usize,


  _phdat :  PhantomData<(K,V)>,
}

const DEF_RANDCACHE_SIZE : usize = 128;

impl<K, V, C : KVCache<K,V>> RandCache<K, V, C> {
  fn new (c : C) -> RandCache<K,V,C> {
    Self::new_add(c, DEF_RANDCACHE_SIZE)
  }
  fn new_add (c : C, rcachebytelength : usize) -> RandCache<K,V,C> {
    let mut r = vec!(0;rcachebytelength);
    thread_rng().fill_bytes(&mut r);
    RandCache {
      cache : c,
      randcache : r,
      randcachepos : 0,
      numratio : 2,
      denratio : 3,
      _phdat : PhantomData,
    } 
  }
}

impl<K,V, C : KVCache<K,V>> KVCache<K, V> for RandCache<K,V,C> {

  fn add_val_c(& mut self, k: K, v: V) {
    self.cache.add_val_c(k,v)
  }
  fn get_val_c<'a>(&'a self, k : &K) -> Option<&'a V> {
    self.cache.get_val_c(k)
  }
  fn has_val_c<'a>(&'a self, k : &K) -> bool {
    self.cache.has_val_c(k)
  }
  fn update_val_c<F>(& mut self, k : &K, f : F) -> Result<bool> where F : FnOnce(& mut V) -> Result<()> {
    self.cache.update_val_c(k,f)
  }
  fn remove_val_c(& mut self, k : &K) -> Option<V> {
    self.cache.remove_val_c(k)
  }
  fn strict_fold_c<'a, B, F>(&'a self, init: B, f: F) -> B where F: Fn(B, (&'a K, &'a V)) -> B, K : 'a, V : 'a {
    self.cache.strict_fold_c(init,f)
  }
  fn fold_c<'a, B, F>(&'a self, init: B, f: F) -> B where F: FnMut(B, (&'a K, &'a V)) -> B, K : 'a, V : 'a {
    self.cache.fold_c(init,f)
  }
  fn map_inplace_c<'a,F>(&'a self, f: F) -> Result<()> where F: FnMut((&'a K, &'a V)) -> Result<()>, K : 'a, V : 'a {
    self.cache.map_inplace_c(f)
  }
  fn len_c (& self) -> usize {
    self.cache.len_c()
  }
  fn new() -> Self {
    RandCache::new(
      C::new()
    )
  }
  fn next_random_values<'a>(&'a mut self, queried_nb : usize) -> Vec<&'a V> where K : 'a {
    let l = self.len_c();
    let randrat = l * self.numratio / self.denratio;
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
    let vlen = if xess == 0 {
      l / 8
    } else {
      (l / 8) + 1
    };
    loop {
      let v = if self.randcachepos + vlen < self.randcache.len() {
        self.randcachepos += vlen;
        &self.randcache[self.randcachepos - vlen .. self.randcachepos]
      } else {
        if vlen > self.randcache.len() {
          // update size
          self.randcache = vec!(0;vlen);
        }
        thread_rng().fill_bytes(&mut self.randcache);
        self.randcachepos = vlen;
        &self.randcache[.. vlen]
      };
      match inner_cache_bv_rand(&self.cache,&v, xess, nb) {
        Some(res) => return res,
        None => {
          warn!("Rerolling random (not enough results)");
        },
      }
    };
  }
}

#[test]
fn test_rand_generic_ca () {
  let h : HashMap<usize, ()> = HashMap::new();
  let mut m = 
    RandCache::new_add (h, 3);
  assert!(0 == m.next_random_values(1).len());
  m.cache.insert(1,());
  assert!(0 == m.next_random_values(1).len());
  m.cache.insert(2,());
  assert!(1 == m.next_random_values(1).len());
  assert!(1 == m.next_random_values(2).len());
  m.cache.insert(3,());
  assert!(1 == m.next_random_values(1).len());
  assert!(2 == m.next_random_values(2).len());
  // fill exactly 8 val
  for i in 4..9 {
    m.cache.insert(i,());
  }
  assert!(8 == m.cache.len());
  assert!(1 == m.next_random_values(1).len());
  assert!(5 == m.next_random_values(8).len());
  m.cache.insert(9,());
  assert!(1 == m.next_random_values(1).len());
  assert!(6 == m.next_random_values(8).len());
}

