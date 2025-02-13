use std::cmp::Ordering;

use smallvec::SmallVec;

use crate::{
    representations::{
        number::{BorrowedNumber, Number},
        Add, Atom, AtomView, Fun, ListSlice, Mul, Num, OwnedAdd, OwnedAtom, OwnedFun, OwnedMul,
        OwnedNum, OwnedPow, OwnedVar, Pow, Var,
    },
    state::{BufferHandle, ResettableBuffer, State, Workspace},
};

impl<'a, P: Atom> AtomView<'a, P> {
    /// Compare two atoms.
    fn cmp(&self, other: &AtomView<'_, P>) -> Ordering {
        match (&self, other) {
            (Self::Num(n1), AtomView::Num(n2)) => n1.get_number_view().cmp(&n2.get_number_view()),
            (Self::Num(_), _) => Ordering::Greater,
            (_, AtomView::Num(_)) => Ordering::Less,
            (Self::Var(v1), AtomView::Var(v2)) => v1.get_name().cmp(&v2.get_name()),
            (Self::Var(_), _) => Ordering::Less,
            (_, AtomView::Var(_)) => Ordering::Greater,
            (Self::Pow(p1), AtomView::Pow(p2)) => {
                let (b1, e1) = p1.get_base_exp();
                let (b2, e2) = p2.get_base_exp();
                b1.cmp(&b2).then_with(|| e1.cmp(&e2))
            }
            (_, AtomView::Pow(_)) => Ordering::Greater,
            (Self::Pow(_), _) => Ordering::Less,
            (Self::Mul(m1), AtomView::Mul(m2)) => {
                let it1 = m1.to_slice();
                let it2 = m2.to_slice();

                let len_cmp = it1.len().cmp(&it2.len());
                if len_cmp != Ordering::Equal {
                    return len_cmp;
                }

                for (t1, t2) in it1.iter().zip(it2.iter()) {
                    let argcmp = t1.cmp(&t2);
                    if argcmp != Ordering::Equal {
                        return argcmp;
                    }
                }

                Ordering::Equal
            }
            (Self::Mul(_), _) => Ordering::Less,
            (_, AtomView::Mul(_)) => Ordering::Greater,
            (Self::Add(a1), AtomView::Add(a2)) => {
                let it1 = a1.to_slice();
                let it2 = a2.to_slice();

                let len_cmp = it1.len().cmp(&it2.len());
                if len_cmp != Ordering::Equal {
                    return len_cmp;
                }

                for (t1, t2) in it1.iter().zip(it2.iter()) {
                    let argcmp = t1.cmp(&t2);
                    if argcmp != Ordering::Equal {
                        return argcmp;
                    }
                }

                Ordering::Equal
            }
            (Self::Add(_), _) => Ordering::Less,
            (_, AtomView::Add(_)) => Ordering::Greater,

            (Self::Fun(f1), AtomView::Fun(f2)) => {
                let name_comp = f1.get_name().cmp(&f2.get_name());
                if name_comp != Ordering::Equal {
                    return name_comp;
                }

                let len_cmp = f1.get_nargs().cmp(&f2.get_nargs());
                if len_cmp != Ordering::Equal {
                    return len_cmp;
                }

                for (arg1, arg2) in f1.iter().zip(f2.iter()) {
                    let argcmp = arg1.cmp(&arg2);
                    if argcmp != Ordering::Equal {
                        return argcmp;
                    }
                }

                Ordering::Equal
            }
        }
    }

    /// Compare factors in a term. `x` and `x^2` are placed next to each other by sorting a power based on the base only.
    fn cmp_factors(&self, other: &AtomView<'_, P>) -> Ordering {
        match (&self, other) {
            (Self::Num(_), AtomView::Num(_)) => Ordering::Equal,
            (Self::Num(_), _) => Ordering::Greater,
            (_, AtomView::Num(_)) => Ordering::Less,

            (Self::Var(v1), AtomView::Var(v2)) => v1.get_name().cmp(&v2.get_name()),
            (Self::Pow(p1), AtomView::Pow(p2)) => {
                // TODO: inline partial_cmp call by creating an inlined version
                p1.get_base().cmp(&p2.get_base())
            }
            (_, AtomView::Pow(p2)) => {
                let base = p2.get_base();
                self.cmp(&base).then(Ordering::Less) // sort x^2*x -> x*x^2
            }
            (Self::Pow(p1), _) => {
                let base = p1.get_base();
                base.cmp(other).then(Ordering::Greater)
            }
            (Self::Var(_), _) => Ordering::Less,
            (_, AtomView::Var(_)) => Ordering::Greater,

            (Self::Mul(_), _) | (_, AtomView::Mul(_)) => {
                unreachable!("Cannot have a submul in a factor");
            }
            (Self::Add(a1), AtomView::Add(a2)) => {
                let it1 = a1.to_slice();
                let it2 = a2.to_slice();

                let len_cmp = it1.len().cmp(&it2.len());
                if len_cmp != Ordering::Equal {
                    return len_cmp;
                }

                for (t1, t2) in it1.iter().zip(it2.iter()) {
                    let argcmp = t1.cmp(&t2);
                    if argcmp != Ordering::Equal {
                        return argcmp;
                    }
                }

                Ordering::Equal
            }
            (Self::Add(_), _) => Ordering::Less,
            (_, AtomView::Add(_)) => Ordering::Greater,

            (Self::Fun(f1), AtomView::Fun(f2)) => {
                // TODO: implement cmp for Fun instead and call that
                let name_comp = f1.get_name().cmp(&f2.get_name());
                if name_comp != Ordering::Equal {
                    return name_comp;
                }

                let len_cmp = f1.get_nargs().cmp(&f2.get_nargs());
                if len_cmp != Ordering::Equal {
                    return len_cmp;
                }

                for (arg1, arg2) in f1.iter().zip(f2.iter()) {
                    let argcmp = arg1.cmp(&arg2);
                    if argcmp != Ordering::Equal {
                        return argcmp;
                    }
                }

                Ordering::Equal
            }
        }
    }

    /// Compare terms in an expression. `x` and `x*2` are placed next to each other.
    pub fn cmp_terms(&self, other: &AtomView<'_, P>) -> Ordering {
        debug_assert!(!matches!(self, Self::Add(_)));
        debug_assert!(!matches!(other, AtomView::Add(_)));
        match (&self, other) {
            (Self::Num(_), AtomView::Num(_)) => Ordering::Equal,
            (Self::Num(_), _) => Ordering::Greater,
            (_, AtomView::Num(_)) => Ordering::Less,

            (Self::Var(v1), AtomView::Var(v2)) => v1.get_name().cmp(&v2.get_name()),
            (Self::Pow(p1), AtomView::Pow(p2)) => {
                let (b1, e1) = p1.get_base_exp();
                let (b2, e2) = p2.get_base_exp();
                b1.cmp(&b2).then_with(|| e1.cmp(&e2))
            }
            (Self::Mul(m1), AtomView::Mul(m2)) => {
                let it1 = m1.to_slice();
                let it2 = m2.to_slice();

                let actual_len1 = if let AtomView::Num(_) = it1.get(it1.len() - 1) {
                    it1.len() - 1
                } else {
                    it1.len()
                };

                let actual_len2 = if let AtomView::Num(_) = it2.get(it2.len() - 1) {
                    it2.len() - 1
                } else {
                    it2.len()
                };

                let len_cmp = actual_len1.cmp(&actual_len2);
                if len_cmp != Ordering::Equal {
                    return len_cmp;
                }

                for (t1, t2) in it1.iter().zip(it2.iter()) {
                    if let AtomView::Num(_) = t1 {
                        break;
                    }
                    if let AtomView::Num(_) = t2 {
                        break;
                    }

                    let argcmp = t1.cmp(&t2);
                    if argcmp != Ordering::Equal {
                        return argcmp;
                    }
                }

                Ordering::Equal
            }
            (Self::Mul(m1), a2) => {
                let it1 = m1.to_slice();
                if it1.len() != 2 {
                    return Ordering::Greater;
                }
                if let AtomView::Num(_) = it1.get(it1.len() - 1) {
                } else {
                    return Ordering::Greater;
                };

                it1.get(0).cmp(a2)
            }
            (a1, AtomView::Mul(m2)) => {
                let it2 = m2.to_slice();
                if it2.len() != 2 {
                    return Ordering::Less;
                }
                if let AtomView::Num(_) = it2.get(it2.len() - 1) {
                } else {
                    return Ordering::Less;
                };

                a1.cmp(&it2.get(0))
            }
            (Self::Var(_), _) => Ordering::Less,
            (_, AtomView::Var(_)) => Ordering::Greater,
            (_, AtomView::Pow(_)) => Ordering::Greater,
            (Self::Pow(_), _) => Ordering::Less,

            (Self::Fun(f1), AtomView::Fun(f2)) => {
                let name_comp = f1.get_name().cmp(&f2.get_name());
                if name_comp != Ordering::Equal {
                    return name_comp;
                }

                let len_cmp = f1.get_nargs().cmp(&f2.get_nargs());
                if len_cmp != Ordering::Equal {
                    return len_cmp;
                }

                for (arg1, arg2) in f1.iter().zip(f2.iter()) {
                    let argcmp = arg1.cmp(&arg2);
                    if argcmp != Ordering::Equal {
                        return argcmp;
                    }
                }

                Ordering::Equal
            }
            (Self::Add(_), _) | (_, AtomView::Add(_)) => unreachable!("Cannot have nested add"),
        }
    }
}

impl<P: Atom> OwnedAtom<P> {
    /// Merge two factors if possible. If this function returns `true`, `self`
    /// will have been updated by the merge from `other` and `other` should be discarded.
    /// If the function return `false`, no merge was possible and no modifications were made.
    fn merge_factors(&mut self, other: &mut Self, helper: &mut Self, state: &State) -> bool {
        // x^a * x^b = x^(a + b)
        if let Self::Pow(p1) = self {
            if let Self::Pow(p2) = other {
                let new_exp = helper.transform_to_num();

                let (base2, exp2) = p2.to_pow_view().get_base_exp();

                // help the borrow checker out by encapsulating base1 and exp1
                {
                    let (base1, exp1) = p1.to_pow_view().get_base_exp();

                    if base1 != base2 {
                        return false;
                    }

                    let AtomView::Num(n) = &exp1 else {
                        unimplemented!("No support for non-numerical powers yet");
                    };
                    new_exp.set_from_view(n);
                }

                let AtomView::Num(n2) = &exp2 else {
                    unimplemented!("No support for non-numerical powers yet")
                };
                new_exp.add(n2, state);

                if new_exp.to_num_view().is_zero() {
                    let num = self.transform_to_num();
                    num.set_from_number(Number::Natural(1, 1));
                } else if new_exp.to_num_view().is_one() {
                    self.from_view(&base2);
                } else {
                    p1.set_from_base_and_exp(base2, AtomView::Num(new_exp.to_num_view()));
                }

                return true;
            }
        }

        // x * x^n = x^(n+1)
        if let Self::Pow(p) = other {
            let pv = p.to_pow_view();
            let (base, exp) = pv.get_base_exp();

            if self.to_view() != base {
                return false;
            }
            let AtomView::Num(n) = &exp else {
                unimplemented!("No support for non-numerical powers yet")
            };
            let num = helper.transform_to_num();

            let new_exp = n
                .get_number_view()
                .add(&BorrowedNumber::Natural(1, 1), state);

            if new_exp.is_zero() {
                let num = self.transform_to_num();
                num.set_from_number(Number::Natural(1, 1));
            } else if Number::Natural(1, 1) == new_exp {
                self.from_view(&base);
            } else {
                num.set_from_number(new_exp);
                let op = self.transform_to_pow();
                op.set_from_base_and_exp(base, AtomView::Num(num.to_num_view()));
            }

            return true;
        }

        // simplify num1 * num2
        if let Self::Num(n1) = self {
            if let Self::Num(n2) = other {
                n1.mul(&n2.to_num_view(), state);
                return true;
            }
            return false;
        }

        // x * x => x^2
        if self.to_view() == other.to_view() {
            // add powers
            let exp = other.transform_to_num();
            exp.set_from_number(Number::Natural(2, 1));

            //let mut a = workspace.get_atom_test_buf();
            let new_pow = helper.transform_to_pow();
            new_pow.set_from_base_and_exp(self.to_view(), AtomView::Num(exp.to_num_view()));

            // overwrite self with the new power view
            let pow_handle = self.transform_to_pow();
            pow_handle.set_from_view(&new_pow.to_pow_view());

            return true;
        }

        false
    }

    /// Merge two terms if possible. If this function returns `true`, `self`
    /// will have been updated by the merge from `other` and `other` should be discarded.
    /// If the function return `false`, no merge was possible and no modifications were made.
    pub fn merge_terms(&mut self, other: &mut Self, helper: &mut Self, state: &State) -> bool {
        if let Self::Num(n1) = self {
            if let Self::Num(n2) = other {
                n1.add(&n2.to_num_view(), state);
                return true;
            }
            return false;
        }

        // compare the non-coefficient part of terms and add the coefficients if they are the same
        if let Self::Mul(m) = self {
            let slice = m.to_mul_view().to_slice();

            let last_elem = slice.get(slice.len() - 1);

            let (non_coeff1, has_coeff) = if let AtomView::Num(_) = &last_elem {
                (slice.get_subslice(0..slice.len() - 1), true)
            } else {
                (m.to_mul_view().to_slice(), false)
            };

            if let Self::Mul(m2) = other {
                let slice2 = m2.to_mul_view().to_slice();
                let last_elem2 = slice2.get(slice2.len() - 1);

                let non_coeff2 = if let AtomView::Num(_) = &last_elem2 {
                    slice2.get_subslice(0..slice2.len() - 1)
                } else {
                    m2.to_mul_view().to_slice()
                };

                if non_coeff1.eq(&non_coeff2) {
                    // TODO: not correct for finite fields!
                    let num = if let AtomView::Num(n) = &last_elem {
                        n.get_number_view()
                    } else {
                        BorrowedNumber::Natural(1, 1)
                    };

                    let new_coeff = if let AtomView::Num(n) = &last_elem2 {
                        num.add(&n.get_number_view(), state)
                    } else {
                        num.add(&BorrowedNumber::Natural(1, 1), state)
                    };

                    // help the borrow checker by dropping all references
                    drop(non_coeff1);
                    drop(non_coeff2);
                    drop(slice2);
                    drop(slice);

                    if new_coeff.is_zero() {
                        let num = self.transform_to_num();
                        num.set_from_number(new_coeff);

                        return true;
                    }

                    let on = helper.transform_to_num();
                    on.set_from_number(new_coeff);

                    if has_coeff {
                        m.replace_last(on.to_num_view().to_view());
                    } else {
                        m.extend(on.to_num_view().to_view());
                    }

                    return true;
                }
            } else {
                if non_coeff1.len() != 1 || other.to_view() != slice.get(0) {
                    return false;
                }

                let new_coeff = if let AtomView::Num(n) = &last_elem {
                    n.get_number_view()
                        .add(&BorrowedNumber::Natural(1, 1), state)
                } else {
                    return false;
                };

                // help the borrow checker by dropping all references
                drop(slice);
                drop(non_coeff1);

                if new_coeff.is_zero() {
                    let num = self.transform_to_num();
                    num.set_from_number(new_coeff);

                    return true;
                }

                let on = helper.transform_to_num();
                on.set_from_number(new_coeff);

                m.replace_last(on.to_num_view().to_view());

                return true;
            }
        } else if let Self::Mul(m) = other {
            let slice = m.to_mul_view().to_slice();

            if slice.len() != 2 {
                return false; // no match
            }

            let last_elem = slice.get(slice.len() - 1);

            if self.to_view() == slice.get(0) {
                let (new_coeff, has_num) = if let AtomView::Num(n) = &last_elem {
                    (
                        n.get_number_view()
                            .add(&BorrowedNumber::Natural(1, 1), state),
                        true,
                    )
                } else {
                    return false; // last elem is not a coefficient
                };

                // help the borrow checker by dropping all references
                drop(slice);

                if new_coeff.is_zero() {
                    let num = self.transform_to_num();
                    num.set_from_number(new_coeff);

                    return true;
                }

                let on = helper.transform_to_num();
                on.set_from_number(new_coeff);

                if has_num {
                    m.replace_last(on.to_num_view().to_view());
                } else {
                    m.extend(on.to_num_view().to_view());
                }

                std::mem::swap(self, other);

                return true;
            }
        } else if self.to_view() == other.to_view() {
            let mul = helper.transform_to_mul();

            let num = other.transform_to_num();
            num.set_from_number(Number::Natural(2, 1));

            mul.extend(self.to_view());
            mul.extend(other.to_view());

            std::mem::swap(self, helper);
            return true;
        };

        false
    }
}

impl<'a, P: Atom> AtomView<'a, P> {
    #[inline(always)]
    pub fn is_dirty(&self) -> bool {
        match self {
            Self::Num(n) => n.is_dirty(),
            Self::Var(_) => false,
            Self::Fun(f) => f.is_dirty(),
            Self::Pow(p) => p.is_dirty(),
            Self::Mul(m) => m.is_dirty(),
            Self::Add(a) => a.is_dirty(),
        }
    }

    /// Normalize an atom.
    pub fn normalize(&self, workspace: &Workspace<P>, state: &State, out: &mut OwnedAtom<P>) {
        if !self.is_dirty() {
            out.from_view(self);
            return;
        }

        match self {
            Self::Mul(t) => {
                let mut atom_test_buf: SmallVec<[BufferHandle<OwnedAtom<P>>; 20]> = SmallVec::new();

                for a in t.iter() {
                    let mut handle = workspace.new_atom();
                    let new_at = handle.get_mut();

                    if a.is_dirty() {
                        a.normalize(workspace, state, new_at);
                    } else {
                        new_at.from_view(&a);
                    }

                    if let OwnedAtom::Mul(mul) = new_at {
                        for c in mul.to_mul_view().iter() {
                            // TODO: remove this copy
                            let mut handle = workspace.new_atom();
                            let child_copy = handle.get_mut();
                            child_copy.from_view(&c);

                            if let AtomView::Num(n) = c {
                                if n.is_one() {
                                    continue;
                                }
                            }

                            atom_test_buf.push(handle);
                        }
                    } else {
                        if let AtomView::Num(n) = handle.get().to_view() {
                            if n.is_one() {
                                continue;
                            }
                        }

                        atom_test_buf.push(handle);
                    }
                }

                atom_test_buf.sort_by(|a, b| a.get().to_view().cmp_factors(&b.get().to_view()));

                if !atom_test_buf.is_empty() {
                    let out_mul = out.transform_to_mul();

                    let mut last_buf = atom_test_buf.remove(0);

                    let mut handle = workspace.new_atom();
                    let helper = handle.get_mut();
                    let mut cur_len = 0;

                    for mut cur_buf in atom_test_buf.drain(..) {
                        if !last_buf
                            .get_mut()
                            .merge_factors(cur_buf.get_mut(), helper, state)
                        {
                            // we are done merging
                            {
                                let v = last_buf.get().to_view();
                                if let AtomView::Num(n) = v {
                                    if !n.is_one() {
                                        out_mul.extend(last_buf.get().to_view());
                                        cur_len += 1;
                                    }
                                } else {
                                    out_mul.extend(last_buf.get().to_view());
                                    cur_len += 1;
                                }
                            }
                            last_buf = cur_buf;
                        }
                    }

                    if cur_len == 0 {
                        out.from_view(&last_buf.get().to_view());
                    } else {
                        out_mul.extend(last_buf.get().to_view());
                    }
                } else {
                    let on = out.transform_to_num();
                    on.set_from_number(Number::Natural(1, 1));
                }
            }
            Self::Num(n) => {
                let normalized_num = n.get_number_view().normalize();
                let nn = out.transform_to_num();
                nn.set_from_number(normalized_num);
            }
            Self::Var(v) => {
                let vv = out.transform_to_var();
                vv.set_from_view(v);
            }
            Self::Fun(f) => {
                let out = out.transform_to_fun();
                out.set_from_name(f.get_name());

                let mut handle = workspace.new_atom();
                let new_at = handle.get_mut();
                for a in f.iter() {
                    if a.is_dirty() {
                        new_at.reset(); // TODO: needed?
                        a.normalize(workspace, state, new_at);
                        out.add_arg(new_at.to_view());
                    } else {
                        out.add_arg(a);
                    }
                }
            }
            Self::Pow(p) => {
                let (base, exp) = p.get_base_exp();

                let mut base_handle = workspace.new_atom();
                let mut exp_handle = workspace.new_atom();

                if base.is_dirty() {
                    base.normalize(workspace, state, base_handle.get_mut());
                } else {
                    // TODO: prevent copy
                    base_handle.get_mut().from_view(&base);
                };

                if exp.is_dirty() {
                    exp.normalize(workspace, state, exp_handle.get_mut());
                } else {
                    // TODO: prevent copy
                    exp_handle.get_mut().from_view(&exp);
                };

                'pow_simplify: {
                    if let AtomView::Num(e) = exp_handle.get().to_view() {
                        if let BorrowedNumber::Natural(0, 1) = &e.get_number_view() {
                            // x^0 = 1
                            let n = out.transform_to_num();
                            n.set_from_number(Number::Natural(1, 1));
                            break 'pow_simplify;
                        } else if let BorrowedNumber::Natural(1, 1) = &e.get_number_view() {
                            // remove power of 1
                            out.from_view(&base_handle.get().to_view());
                            break 'pow_simplify;
                        } else if let AtomView::Num(n) = base_handle.get().to_view() {
                            // simplify a number to a numerical power
                            let (new_base_num, new_exp_num) =
                                n.get_number_view().pow(&e.get_number_view(), state);

                            if let Number::Natural(1, 1) = &new_exp_num {
                                let out = out.transform_to_num();
                                out.set_from_number(new_base_num);
                                break 'pow_simplify;
                            }

                            let nb = base_handle.get_mut().transform_to_num();
                            nb.set_from_number(new_base_num);

                            let ne = exp_handle.get_mut().transform_to_num();
                            ne.set_from_number(new_exp_num);
                        } else if let AtomView::Pow(p_base) = base_handle.get().to_view() {
                            // simplify x^2^3
                            let (p_base_base, p_base_exp) = p_base.get_base_exp();
                            if let AtomView::Num(n) = p_base_exp {
                                let new_exp = n.get_number_view().mul(&e.get_number_view(), state);

                                if let Number::Natural(1, 1) = &new_exp {
                                    out.from_view(&p_base_base);
                                    break 'pow_simplify;
                                }

                                let ne = exp_handle.get_mut().transform_to_num();
                                ne.set_from_number(new_exp);

                                let out = out.transform_to_pow();
                                out.set_from_base_and_exp(p_base_base, exp_handle.get().to_view());

                                break 'pow_simplify;
                            }
                        } else if let AtomView::Mul(_) = base_handle.get().to_view() {
                            // TODO: turn (x*y)^2 into x^2*y^2?
                            // for now, expand() needs to be used
                        }
                    }

                    let out = out.transform_to_pow();
                    out.set_from_base_and_exp(
                        base_handle.get().to_view(),
                        exp_handle.get().to_view(),
                    );
                }
            }
            Self::Add(a) => {
                let mut atom_test_buf: SmallVec<[BufferHandle<OwnedAtom<P>>; 20]> = SmallVec::new();

                for a in a.iter() {
                    let mut handle = workspace.new_atom();
                    let new_at = handle.get_mut();

                    if a.is_dirty() {
                        a.normalize(workspace, state, new_at);
                    } else {
                        new_at.from_view(&a);
                    }

                    if let OwnedAtom::Add(new_add) = new_at {
                        for c in new_add.to_add_view().iter() {
                            // TODO: remove this copy
                            let mut handle = workspace.new_atom();
                            let child_copy = handle.get_mut();
                            child_copy.from_view(&c);

                            if let AtomView::Num(n) = c {
                                if n.is_zero() {
                                    continue;
                                }
                            }

                            atom_test_buf.push(handle);
                        }
                    } else {
                        if let AtomView::Num(n) = handle.get().to_view() {
                            if n.is_zero() {
                                continue;
                            }
                        }
                        atom_test_buf.push(handle);
                    }
                }

                atom_test_buf.sort_by(|a, b| a.get().to_view().cmp_terms(&b.get().to_view()));

                if !atom_test_buf.is_empty() {
                    let out_add = out.transform_to_add();

                    let mut last_buf = atom_test_buf.remove(0);

                    let mut handle = workspace.new_atom();
                    let helper = handle.get_mut();
                    let mut cur_len = 0;

                    for mut cur_buf in atom_test_buf.drain(..) {
                        if !last_buf
                            .get_mut()
                            .merge_terms(cur_buf.get_mut(), helper, state)
                        {
                            // we are done merging
                            {
                                let v = last_buf.get().to_view();
                                if let AtomView::Num(n) = v {
                                    if !n.is_zero() {
                                        out_add.extend(last_buf.get().to_view());
                                        cur_len += 1;
                                    }
                                } else {
                                    out_add.extend(last_buf.get().to_view());
                                    cur_len += 1;
                                }
                            }
                            last_buf = cur_buf;
                        }
                    }

                    if cur_len == 0 {
                        out.from_view(&last_buf.get().to_view());
                    } else {
                        out_add.extend(last_buf.get().to_view());
                    }
                } else {
                    let on = out.transform_to_num();
                    on.set_from_number(Number::Natural(0, 1));
                }
            }
        }
    }
}
