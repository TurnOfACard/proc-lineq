#![warn(clippy::panic, clippy::str_to_string, clippy::panicking_unwrap)]

use proc_macro2::{Ident, Span};
use syn::spanned::Spanned;
use syn::{parse_quote, BinOp, Expr, ExprBinary, ExprClosure, ExprPath, Token};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("only a subset of binary operators are allowed")]
    BinOp,
    #[error("cannot have multiple of the target variable")]
    Multiple,
    #[error("solve_for not found")]
    NoSolveFor,
    #[error("unexpected identifier")]
    UnexpectedIdentifier,
    #[error("used unrecognised features")]
    Validation,
}

/// Stores the variables and the current state of the calculation
///
/// Call [`solve`] to build an output expression.
pub struct ClosureInverter {
    target_expr: Box<Expr>,
    solve_for: Ident,
    target_ident: Ident,
}

impl ClosureInverter {
    pub fn new(solve_for: Ident, target_ident: Ident) -> Self {
        Self {
            target_expr: Box::new(Expr::Path(ExprPath {
                attrs: vec![],
                qself: None,
                path: parse_quote!(#target_ident),
            })),
            solve_for,
            target_ident,
        }
    }

    /// Returns true if is a valid expression to invert.
    fn validate_expr(e: &Expr) -> bool {
        match e {
            Expr::Binary(b) => Self::validate_expr(&b.left) && Self::validate_expr(&b.right),
            Expr::Lit(_) | Expr::Path(_) => true,
            _ => false,
        }
    }

    /// Parses a closure returning the inverse if possible.
    pub fn solve(mut self, closure: &ExprClosure) -> Result<ExprClosure, ParseError> {
        if Self::validate_expr(&closure.body) {
            self.parse_expr(*closure.body.clone())?;

            let target_expr = self.target_expr;
            let target_ident = self.target_ident;
            let c: ExprClosure = parse_quote!( |#target_ident| #target_expr);
            Ok(c)
        } else {
            Err(ParseError::Validation)
        }
    }

    /// Recursive call which stops when Expr only contains the target path
    fn parse_expr(&mut self, e: Expr) -> Result<(), ParseError> {
        let e_span = e.span();
        match e {
            Expr::Binary(b) => {
                let left = Self::check_contains_target(&b.left, &self.solve_for);
                let right = Self::check_contains_target(&b.right, &self.solve_for);
                let inverted_op = inverse_bin_op(&b.op, &e_span)?;

                // Parenthesize expression
                let target_expr = &self.target_expr;
                match (left, right) {
                    (true, false) => {
                        self.target_expr = Self::build_expr_binary(
                            Self::parenthesize(target_expr, &inverted_op)?,
                            inverted_op,
                            b.right.clone(),
                        );
                        self.parse_expr(*b.left)
                    }
                    (false, true) => match &b.op {
                        BinOp::Add(_) | BinOp::Mul(_) => {
                            self.target_expr = Self::build_expr_binary(
                                Self::parenthesize(target_expr, &inverted_op)?,
                                inverted_op,
                                b.left.clone(),
                            );
                            self.parse_expr(*b.right)
                        }
                        BinOp::Sub(_) | BinOp::Div(_) => {
                            self.target_expr = Self::build_expr_binary(
                                b.left.clone(),
                                b.op,
                                Self::parenthesize(target_expr, &b.op)?,
                            );
                            self.parse_expr(*b.right)
                        }
                        _ => Err(ParseError::BinOp),
                    },
                    (true, true) => Err(ParseError::Multiple),
                    (false, false) => Err(ParseError::NoSolveFor),
                }
            }
            Expr::Path(p) => {
                if Self::parse_path(&p, &self.solve_for) {
                    Ok(())
                } else {
                    Err(ParseError::UnexpectedIdentifier)
                }
            }
            _ => unimplemented!(),
        }
    }

    fn build_expr_binary(left: Box<Expr>, op: BinOp, right: Box<Expr>) -> Box<Expr> {
        Box::from({
            Expr::Binary(ExprBinary {
                attrs: vec![],
                left,
                op,
                right,
            })
        })
    }

    fn check_contains_target(e: &Expr, target: &Ident) -> bool {
        match e {
            Expr::Binary(b) => {
                Self::check_contains_target(&b.left, target)
                    || Self::check_contains_target(&b.right, target)
            }
            Expr::Lit(_) => false,
            Expr::Paren(_) => unimplemented!(),
            Expr::Path(p) => Self::parse_path(p, target),
            Expr::Unary(_) => unimplemented!(),
            _ => unimplemented!(),
        }
    }

    fn parse_path(p: &ExprPath, target: &Ident) -> bool {
        if p.attrs.is_empty() && p.qself.is_none() {
            p.path.is_ident(target)
        } else {
            false
        }
    }

    // Adds parentheses if required
    fn parenthesize(e: &Expr, target_op: &BinOp) -> Result<Box<Expr>, ParseError> {
        match e {
            Expr::Lit(_) | Expr::Path(_) => Ok(Box::new(e.clone())),
            _ => match target_op {
                BinOp::Add(_) | BinOp::Sub(_) => Ok(Box::new(e.clone())),
                BinOp::Mul(_) | BinOp::Div(_) => Ok(parse_quote!( (#e))),
                _ => Err(ParseError::BinOp),
            },
        }
    }
}

fn inverse_bin_op(op: &BinOp, dummy_span: &Span) -> Result<BinOp, ParseError> {
    match op {
        BinOp::Add(_) => Ok(BinOp::Sub(Token![-](*dummy_span))),
        BinOp::Sub(_) => Ok(BinOp::Add(Token![+](*dummy_span))),
        BinOp::Mul(_) => Ok(BinOp::Div(Token![/](*dummy_span))),
        BinOp::Div(_) => Ok(BinOp::Mul(Token![*](*dummy_span))),
        _ => Err(ParseError::BinOp),
    }
}

#[cfg(test)]
mod tests {
    use proc_lineq_derive::ClosureInverter;

    // All tests currently test usize types only. Can be expanded in the future.

    #[test]
    fn invert_basic_addition() {
        #[derive(ClosureInverter)]
        #[invert("|| a + 2")]
        struct Test;
        assert_eq!(Test::calculate(5), 3);
        assert_eq!(Test::calculate(3), 1);

        #[derive(ClosureInverter)]
        #[invert("|| a + 5")]
        struct Test2;
        assert_eq!(Test2::calculate(5), 0);
        assert_eq!(Test2::calculate(10), 5);
    }

    #[test]
    fn invert_basic_subtraction() {
        #[derive(ClosureInverter)]
        #[invert("|| a - 2")]
        struct Test;
        assert_eq!(Test::calculate(5), 7);
        assert_eq!(Test::calculate(3), 5);

        #[derive(ClosureInverter)]
        #[invert("|| a - 5")]
        struct Test2;
        assert_eq!(Test2::calculate(5), 10);
        assert_eq!(Test2::calculate(10), 15);
    }

    #[test]
    fn invert_basic_multiplication() {
        #[derive(ClosureInverter)]
        #[invert("|| a * 2")]
        struct Test;
        assert_eq!(Test::calculate(5), 2);
        assert_eq!(Test::calculate(3), 1);

        #[derive(ClosureInverter)]
        #[invert("|| a * 5")]
        struct Test2;
        assert_eq!(Test2::calculate(5), 1);
        assert_eq!(Test2::calculate(10), 2);
    }

    #[test]
    fn invert_basic_division() {
        #[derive(ClosureInverter)]
        #[invert("|| a / 2")]
        struct Test;
        assert_eq!(Test::calculate(5), 10);
        assert_eq!(Test::calculate(3), 6);

        #[derive(ClosureInverter)]
        #[invert("|| a / 5")]
        struct Test2;
        assert_eq!(Test2::calculate(5), 25);
        assert_eq!(Test2::calculate(10), 50);
    }

    #[test]
    fn invert_complex_operators() {
        #[derive(ClosureInverter)]
        #[invert("|| a / 2 + 2")]
        struct TestComplex;
        assert_eq!(TestComplex::calculate(5), 6);
        assert_eq!(TestComplex::calculate(3), 2);

        #[derive(ClosureInverter)]
        #[invert("|| a / 5 - 3 * 2")]
        struct TestComplex2;
        assert_eq!(TestComplex2::calculate(5), 55);
        assert_eq!(TestComplex2::calculate(10), 80);

        #[derive(ClosureInverter)]
        #[invert("|| 2 + a - 3 * 2")]
        struct TestComplex3;

        assert_eq!(TestComplex3::calculate(5), 9);
        assert_eq!(TestComplex3::calculate(10), 14);

        #[derive(ClosureInverter)]
        #[invert("|| 200 - a * 2 + 3 * 2")]
        struct TestComplex4;

        assert_eq!(TestComplex4::calculate(20), 87);
        assert_eq!(TestComplex4::calculate(1), 96);

        #[derive(ClosureInverter)]
        #[invert("|| 10 - 2 * a + 4 / 2")]
        struct TestComplex5;

        assert_eq!(TestComplex5::calculate(2), 3);
        assert_eq!(TestComplex5::calculate(1), 3);

        #[derive(ClosureInverter)]
        #[invert("|| 33 + 4 * 2 - 100 / a")]
        struct TestComplex6;

        assert_eq!(TestComplex6::calculate(21), 5);
        assert_eq!(TestComplex6::calculate(31), 10);
    }
}
