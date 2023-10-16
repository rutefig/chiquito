# Compute the factorial of a number n
# The factorial of n is the product of all positive integers less than or equal to n 
# For example, 5! = 5 x 4 x 3 x 2 x 1 = 120
# 1 = 1
# 2 = 2 * 1 = 2
# 3 = 3 * 2 * 1 = 6
# 4 = 4 * 3 * 2 * 1 = 24

######## SIGNALS ##########
###  a  ###  b  ###  c  ###
###  0  ###    ###  1  ###
###  1  ###  1  ###  1  ###
###  2  ###  1  ###  2  ###
###  3  ###  2  ###  6  ###
###  4  ###  6  ###  24  ###
###  5  ###  24  ###  ...  ###
###  6  ###  ...  ###  ...  ###

## Constraints
# 0! = 1
# a * b == c
# c == b.next
# a + 1 == a.next 

from chiquito.dsl import Circuit, StepType
from chiquito.cb import eq
from chiquito.util import F
from chiquito.chiquito_ast import Last

NUMBER_STEPS = 10

class Padding(StepType):
    def setup(self):
        self.transition(eq(self.circuit.b, self.circuit.b.next()))
        self.transition(eq(self.circuit.n, self.circuit.n.next()))
    def wg(self, args):
        a_value, b_value, n_value = args
        self.assign(self.circuit.a, F(a_value))
        self.assign(self.circuit.b, F(b_value))
        self.assign(self.circuit.n, F(n_value))

class FactorialFirstStep(StepType):
    def setup(self):
        self.c = self.internal("c")
        self.constr(eq(self.circuit.a, 0))
        self.constr(eq(self.c, 1))
        self.transition(eq(self.c, self.circuit.b.next()))
        self.transition(eq(self.circuit.a + 1, self.circuit.a.next()))
        self.transition(eq(self.circuit.n, self.circuit.n.next()))
    def wg(self, args):
        a_value, n_value = args
        self.assign(self.circuit.a, F(a_value))
        self.assign(self.circuit.b, F(1))
        self.assign(self.c, F(1))
        self.assign(self.circuit.n, F(n_value))

class FactorialStep(StepType):
    def setup(self):
        self.c = self.internal("c")
        self.constr(eq(self.circuit.a * self.circuit.b, self.c))
        self.transition(eq(self.c, self.circuit.b.next()))
        self.transition(eq(self.circuit.n, self.circuit.n.next()))

    def wg(self, args):
        a_value, b_value, n_value = args
        self.assign(self.circuit.a, F(a_value))
        self.assign(self.circuit.b, F(b_value))
        self.assign(self.c, F(a_value * b_value))
        self.assign(self.circuit.n, F(n_value))

class Factorial(Circuit):
    def setup(self):
        self.a = self.forward("a")
        self.b = self.forward("b")
        self.n = self.forward("n")

        self.factorial_first_step = self.step_type(FactorialFirstStep(self, "factorial_first_step"))
        self.factorial_step = self.step_type(FactorialStep(self, "factorial_step"))
        self.padding = self.step_type(Padding(self, "padding"))

        self.pragma_num_steps(NUMBER_STEPS)
        self.pragma_first_step(self.factorial_first_step)
        self.pragma_last_step(self.padding)

        self.expose(self.b, Last())
        self.expose(self.n, Last())

    def trace(self, n):
        self.add(self.factorial_first_step, (0, n))
        self.add(self.factorial_step, (1, 1, n))

        a = 2
        b = 1
        for i in range(2, n):
            self.add(self.factorial_step, (a, b, n))
            b = b * a
            a += 1
        while self.needs_padding():
            self.add(self.padding, (a, b, n))


factorial = Factorial()
# print(factorial)
factorial_witness = factorial.gen_witness(9)
# print(factorial_witness)
factorial.halo2_mock_prover(factorial_witness)