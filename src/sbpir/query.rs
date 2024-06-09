use std::{
    fmt::Debug,
    marker::PhantomData,
    ops::{Add, Mul, Neg, Sub},
};

use crate::{
    frontend::dsl::StepTypeHandler,
    sbpir::{
        FixedSignal, ForwardSignal, ImportedHalo2Advice, ImportedHalo2Fixed, InternalSignal,
        SharedSignal,
    },
    util::UUID,
};

use crate::poly::{Expr, ToExpr};

use super::PIR;

// Queriable
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Queriable<F> {
    Internal(InternalSignal),
    Forward(ForwardSignal, bool),
    Shared(SharedSignal, i32),
    Fixed(FixedSignal, i32),
    StepTypeNext(StepTypeHandler),
    Halo2AdviceQuery(ImportedHalo2Advice, i32),
    Halo2FixedQuery(ImportedHalo2Fixed, i32),
    #[allow(non_camel_case_types)]
    _unaccessible(PhantomData<F>),
}

impl<F> Debug for Queriable<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.annotation())
    }
}

impl<F> Queriable<F> {
    /// Call `next` function on a `Querible` forward signal to build constraints for forward
    /// signal with rotation. Cannot be called on an internal signal and must be used within a
    /// `transition` constraint. Returns a new `Queriable` forward signal with rotation.
    pub fn next(&self) -> Queriable<F> {
        use Queriable::*;
        match self {
            Forward(s, rot) => {
                if !*rot {
                    Forward(*s, true)
                } else {
                    panic!("jarrl: cannot rotate next(forward)")
                }
            }
            Shared(s, rot) => Shared(*s, rot + 1),
            Fixed(s, rot) => Fixed(*s, rot + 1),
            Halo2AdviceQuery(s, rot) => Halo2AdviceQuery(*s, rot + 1),
            Halo2FixedQuery(s, r) => Halo2FixedQuery(*s, r + 1),
            _ => panic!("can only next a forward, shared, fixed, or halo2 column"),
        }
    }

    /// Call `prev` function on a `Querible` shared signal to build constraints for shared
    /// signal that decreases rotation by 1. Must be called on a shared signal and used within a
    /// `transition` constraint. Returns a new `Queriable` shared signal with positive or
    /// negative rotation.
    pub fn prev(&self) -> Queriable<F> {
        use Queriable::*;
        match self {
            Shared(s, rot) => Shared(*s, rot - 1),
            Fixed(s, rot) => Fixed(*s, rot - 1),
            _ => panic!("can only prev a shared or fixed column"),
        }
    }

    /// Call `rot` function on a `Querible` shared signal to build constraints for shared signal
    /// with arbitrary rotation. Must be called on a shared signal and used within a
    /// `transition` constraint. Returns a new `Queriable` shared signal with positive or
    /// negative rotation.
    pub fn rot(&self, rotation: i32) -> Queriable<F> {
        use Queriable::*;
        match self {
            Shared(s, rot) => Shared(*s, rot + rotation),
            Fixed(s, rot) => Fixed(*s, rot + rotation),
            _ => panic!("can only rot a shared or fixed column"),
        }
    }

    pub fn uuid(&self) -> UUID {
        match self {
            Queriable::Internal(s) => s.uuid(),
            Queriable::Forward(s, _) => s.uuid(),
            Queriable::Shared(s, _) => s.uuid(),
            Queriable::Fixed(s, _) => s.uuid(),
            Queriable::StepTypeNext(s) => s.uuid(),
            Queriable::Halo2AdviceQuery(s, _) => s.uuid(),
            Queriable::Halo2FixedQuery(s, _) => s.uuid(),
            Queriable::_unaccessible(_) => panic!("jarrl wrong queriable type"),
        }
    }

    pub fn annotation(&self) -> String {
        match self {
            Queriable::Internal(s) => s.annotation.to_string(),
            Queriable::Forward(s, rot) => {
                if !rot {
                    s.annotation.to_string()
                } else {
                    format!("next({})", s.annotation)
                }
            }
            Queriable::Shared(s, rot) => {
                if *rot != 0 {
                    format!("{}(rot {})", s.annotation, rot)
                } else {
                    s.annotation.to_string()
                }
            }
            Queriable::Fixed(s, rot) => {
                if *rot != 0 {
                    format!("{}(rot {})", s.annotation, rot)
                } else {
                    s.annotation.to_string()
                }
            }
            Queriable::StepTypeNext(s) => s.annotation.to_string(),
            Queriable::Halo2AdviceQuery(s, rot) => {
                if *rot != 0 {
                    format!("{}(rot {})", s.annotation, rot)
                } else {
                    s.annotation.to_string()
                }
            }
            Queriable::Halo2FixedQuery(s, rot) => {
                if *rot != 0 {
                    format!("{}(rot {})", s.annotation, rot)
                } else {
                    s.annotation.to_string()
                }
            }
            Queriable::_unaccessible(_) => todo!(),
        }
    }
}

impl<F: Clone> ToExpr<F, Queriable<F>> for Queriable<F> {
    fn expr(&self) -> PIR<F> {
        Expr::Query((*self).clone())
    }
}

impl<F: Clone, RHS: Into<PIR<F>>> Add<RHS> for Queriable<F> {
    type Output = PIR<F>;

    fn add(self, rhs: RHS) -> Self::Output {
        self.expr() + rhs
    }
}

impl<F: Clone, RHS: Into<PIR<F>>> Sub<RHS> for Queriable<F> {
    type Output = PIR<F>;

    fn sub(self, rhs: RHS) -> Self::Output {
        self.expr() - rhs
    }
}

impl<F: Clone, RHS: Into<PIR<F>>> Mul<RHS> for Queriable<F> {
    type Output = PIR<F>;

    fn mul(self, rhs: RHS) -> Self::Output {
        self.expr() * rhs
    }
}

impl<F: Clone> Neg for Queriable<F> {
    type Output = PIR<F>;

    fn neg(self) -> Self::Output {
        self.expr().neg()
    }
}

impl<F> From<Queriable<F>> for PIR<F> {
    fn from(value: Queriable<F>) -> Self {
        Expr::Query(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::halo2curves::bn256::Fr;

    #[test]
    fn test_expr_fmt() {
        let a: Fr = 10.into();
        let b: Fr = 20.into();

        let expr1: Expr<Fr, Queriable<Fr>> = Expr::Const(a);
        assert_eq!(format!("{:?}", expr1), "0xa");

        let expr2: Expr<Fr, Queriable<Fr>> = Expr::Sum(vec![Expr::Const(a), Expr::Const(b)]);
        assert_eq!(format!("{:?}", expr2), "(0xa + 0x14)");

        let expr3: Expr<Fr, Queriable<Fr>> = Expr::Mul(vec![Expr::Const(a), Expr::Const(b)]);
        assert_eq!(format!("{:?}", expr3), "(0xa * 0x14)");

        let expr4: Expr<Fr, Queriable<Fr>> = Expr::Neg(Box::new(Expr::Const(a)));
        assert_eq!(format!("{:?}", expr4), "(-0xa)");

        let expr5: Expr<Fr, Queriable<Fr>> = Expr::Pow(Box::new(Expr::Const(a)), 2);
        assert_eq!(format!("{:?}", expr5), "(0xa)^2");
    }

    #[test]
    fn test_next_for_forward_signal() {
        let forward_signal = ForwardSignal {
            id: 0,
            phase: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Forward(forward_signal, false);
        let next_queriable = queriable.next();

        assert_eq!(next_queriable, Queriable::Forward(forward_signal, true));
    }

    #[test]
    #[should_panic(expected = "jarrl: cannot rotate next(forward)")]
    fn test_next_for_forward_signal_panic() {
        let forward_signal = ForwardSignal {
            id: 0,
            phase: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Forward(forward_signal, true);
        let _ = queriable.next(); // This should panic
    }

    #[test]
    fn test_next_for_shared_signal() {
        let shared_signal = SharedSignal {
            id: 0,
            phase: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Shared(shared_signal, 0);
        let next_queriable = queriable.next();

        assert_eq!(next_queriable, Queriable::Shared(shared_signal, 1));
    }

    #[test]
    fn test_next_for_fixed_signal() {
        let fixed_signal = FixedSignal {
            id: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Fixed(fixed_signal, 0);
        let next_queriable = queriable.next();

        assert_eq!(next_queriable, Queriable::Fixed(fixed_signal, 1));
    }

    #[test]
    #[should_panic(expected = "can only next a forward, shared, fixed, or halo2 column")]
    fn test_next_for_internal_signal_panic() {
        let internal_signal = InternalSignal {
            id: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Internal(internal_signal);
        let _ = queriable.next(); // This should panic
    }

    #[test]
    fn test_prev_for_shared_signal() {
        let shared_signal = SharedSignal {
            id: 0,
            phase: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Shared(shared_signal, 1);
        let prev_queriable = queriable.prev();

        assert_eq!(prev_queriable, Queriable::Shared(shared_signal, 0));
    }

    #[test]
    fn test_prev_for_fixed_signal() {
        let fixed_signal = FixedSignal {
            id: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Fixed(fixed_signal, 1);
        let prev_queriable = queriable.prev();

        assert_eq!(prev_queriable, Queriable::Fixed(fixed_signal, 0));
    }

    #[test]
    #[should_panic(expected = "can only prev a shared or fixed column")]
    fn test_prev_for_forward_signal_panic() {
        let forward_signal = ForwardSignal {
            id: 0,
            phase: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Forward(forward_signal, true);
        let _ = queriable.prev(); // This should panic
    }

    #[test]
    #[should_panic(expected = "can only prev a shared or fixed column")]
    fn test_prev_for_internal_signal_panic() {
        let internal_signal = InternalSignal {
            id: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Internal(internal_signal);
        let _ = queriable.prev(); // This should panic
    }

    #[test]
    fn test_rot_for_shared_signal() {
        let shared_signal = SharedSignal {
            id: 0,
            phase: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Shared(shared_signal, 1);
        let rot_queriable = queriable.rot(2);

        assert_eq!(rot_queriable, Queriable::Shared(shared_signal, 3));
    }

    #[test]
    fn test_rot_for_fixed_signal() {
        let fixed_signal = FixedSignal {
            id: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Fixed(fixed_signal, 1);
        let rot_queriable = queriable.rot(2);

        assert_eq!(rot_queriable, Queriable::Fixed(fixed_signal, 3));
    }

    #[test]
    #[should_panic(expected = "can only rot a shared or fixed column")]
    fn test_rot_for_forward_signal_panic() {
        let forward_signal = ForwardSignal {
            id: 0,
            phase: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Forward(forward_signal, true);
        let _ = queriable.rot(2); // This should panic
    }

    #[test]
    #[should_panic(expected = "can only rot a shared or fixed column")]
    fn test_rot_for_internal_signal_panic() {
        let internal_signal = InternalSignal {
            id: 0,
            annotation: "",
        };
        let queriable: Queriable<Fr> = Queriable::Internal(internal_signal);
        let _ = queriable.rot(2); // This should panic
    }
}
