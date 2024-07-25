const MAX_INT: u64 = i32::MAX as u64;

/// Optimus is used to encode and decode integers using Knuth's Hashing Algorithm.
/// This is a rust port of the https://github.com/pjebs/optimus-go/blob/master/optimus.go implementation.
pub struct Optimus {
    prime: u64,
    mod_inverse: u64,
    random: u64,
}

impl Optimus {
    pub fn new(prime: u64, random: u64) -> Self {
        assert!(random < MAX_INT);
        assert!(prime < MAX_INT);
        assert!(primal_check::miller_rabin(prime));
        Self {
            prime,
            mod_inverse: Self::mod_inverse(prime, MAX_INT + 1),
            random,
        }
    }

    /// Encode is used to encode n using Knuth's hashing algorithm.
    pub fn encode(&self, n: u64) -> u64 {
        ((n * self.prime) & MAX_INT) ^ self.random
    }

    /// Decode is used to decode n back to the original.
    pub fn decode(&self, n: u64) -> u64 {
        ((n ^ self.random) * self.mod_inverse) & MAX_INT
    }

    /// Finds the greatest common denominator of two integers *a* and *b*, and two
    /// integers *x* and *y* such that *ax* + *by* is the greatest common
    /// denominator of *a* and *b* (Bézout coefficients).
    fn extended_gcd(a: i64, b: i64) -> (i64, i64, i64) {
        let mut x = 1;
        let mut y = 0;

        if b == 0 {
            (a, x, y)
        } else {
            let mut new_x = 0;
            let mut new_y = 1;
            let mut new_r = b;
            let mut r = a;
            while new_r != 0 {
                let quotient = r / new_r;

                let tmp = r;
                r = new_r;
                new_r = tmp - quotient * new_r;

                let tmp = x;
                x = new_x;
                new_x = tmp - quotient * new_x;

                let tmp = y;
                y = new_y;
                new_y = tmp - quotient * new_y;
            }

            (r, x, y)
        }
    }

    /// Return the  modular inverse
    /// If a and n are relatively prime, there exist integers x and y such that
    /// a * x + n * y = 1, and such integers may be found using the Euclidean
    /// algorithm. Considering this equation modulo n, it follows that a * x = 1; i.e.,
    /// x = a^(-1)(mod n).
    /// Source: https://github.com/wandering007/algorithms/blob/master/extended_gcd.cpp
    fn mod_inverse(a: u64, m: u64) -> u64 {
        let a = a as i64;
        let m = m as i64;
        let (r, x, _) = Self::extended_gcd(a, m);
        assert!(r <= 1);
        ((x % m + m) % m) as u64
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use shine_test::test;

    #[test]
    fn encode_decode() {
        let test_case = [
            (309779747, 49560203, 57733611),
            (309779747, 49560203, 57733612),
            (684934207, 1505143743, 846034763),
            (743534599, 1356791223, 1336232185),
            (54661037, 1342843941, 576322863),
            (198194831, 229517423, 459462336),
        ];

        for (prime, mod_inverse, random) in test_case {
            log::info!("prime: {prime}, random: {random}");
            let opt = Optimus::new(prime, random);

            assert_eq!(opt.mod_inverse, mod_inverse);

            for i in 0..10_000 {
                assert_eq!(opt.decode(opt.encode(i)), i)
            }
            for i in 13_478_000..14_479_100 {
                assert_eq!(opt.decode(opt.encode(i)), i)
            }
        }
    }
}
