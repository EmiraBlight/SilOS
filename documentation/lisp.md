# Lisp Dialect Reference Manual

This document outlines the syntax, data types, and core features of the kernel's Lisp dialect. The language is designed for execution within a custom operating system environment, providing direct interaction with system calls and memory mutation.



## 1. Syntax Overview
* **Statements:** All top-level statements must end with a semicolon `;`.
* **Comments:** Comments are enclosed within pound signs `# ... #`.
* **Quoting:** Prevent evaluation of a list using the quote operator `'(...)` or the explicit `(quote (...))` form.



## 2. Data Types
* **Numbers:** Parsed as 64-bit floating-point numbers (`f64`). For bitwise operations and map keys, they are internally cast to 64-bit signed integers (`i64`). For future versions this will be implemented as a i64
* **Booleans:** Standard `true` and `false` values.
* **Strings:** Text enclosed in double quotes, e.g., `"hello"`.
* **Lists:** Dynamically sized arrays containing any valid language type.
* **Maps:** Key-value data structures. Keys are restricted to Strings, Numbers, and Booleans to ensure safe ordering in memory.


## 3. Variables and Scoping

### Global Variables
Variables are defined globally using the `def` keyword.
```lisp
(def a 10);
(def b "hello");
(def my_list (list 1 2 3));
```

### Local Scoping
Variables can be strictly scoped to a specific block using the `let` keyword.
```lisp
(let (x 5 y 10) (+ x y)); 
```

### Command Line Arguments
When a script is executed via the shell, command-line arguments are automatically injected into the global environment.
* **Numbers:** Accessed via `n0`, `n1`, `n2`, etc. (Default: `0`)
* **Booleans:** Accessed via `b0`, `b1`, `b2`, etc. (Default: `false`)
* In the future this will be done via args, which will be a global list variable with the arguments

## 4. Operators

### Mathematical Operators
| Operator | Description | Example |
| :--- | :--- | :--- |
| `+` | Addition | `(+ 10 2)` evaluates to `12` |
| `-` | Subtraction | `(- 10 2)` evaluates to `8` |
| `*` | Multiplication | `(* 1 2 3)` evaluates to `6` |
| `/` | Division (truncates towards zero) | `(/ 10 5 4)` evaluates to `0` |

### Bitwise Operators
*Note: Floating-point numbers are cast to integers prior to bitwise evaluation.*
| Operator | Description | Example |
| :--- | :--- | :--- |
| `&` | Bitwise AND | `(& 4 5)` evaluates to `4` |
| `\|` | Bitwise OR | `(\| 4 5)` evaluates to `5` |
| `^` | Bitwise XOR | `(^ 4 5)` evaluates to `1` |
| `<<` | Left Shift | `(<< 4 2)` evaluates to `16` |
| `>>` | Right Shift | `(>> 4 1)` evaluates to `2` |

### Relational and Logical Operators
Logical operators support short-circuit evaluation.
| Operator | Description | Example |
| :--- | :--- | :--- |
| `=` | Equal | `(= 10 10)` |
| `!=` | Not Equal | `(!= 5 4)` |
| `>`, `<`, `>=`, `<=` | Relational | `(> 10 5)` |
| `and` | Logical AND | `(and (= 1 1) (> 5 2))` |
| `or` | Logical OR | `(or (= 1 2) (= 2 2))` |
| `not` | Logical NOT | `(not (= 1 2))` |



## 5. Control Flow

### If-Else Statements
Standard conditional execution.
```lisp
(if (= 10 10) 
    (sys echo "equal") 
    (sys echo "not equal")
);
```

### Do Blocks
Groups multiple expressions to be evaluated sequentially. It returns the result of the final evaluated expression. This is heavily utilized within functions and loops.
```lisp
(do
    (sys echo "First step")
    (sys echo "Second step")
    (+ 1 2) # Returns 3 #
)
```

### While Loops
Iterates as long as the provided condition evaluates to `true`.
```lisp
(def a 5);
(for (> a 0)
    (do
        (sys echo a)
        (def a (- a 1))
    )
);
```



## 6. Functions
Functions are defined using the `fn` keyword and bound to a variable via `def`. They support closures and capture their surrounding environment.
```lisp
(def addthree (fn (a b c) (+ a b c)));
```

For functions requiring multiple statements, utilize a `do` block:
```lisp
(def add_and_log (fn (x y) 
    (do 
        (sys echo "Adding:")
        (+ x y)
    )
));
```

## 7. Collections and Memory Manipulation

### Lists and Strings
```lisp
(def a []);                 # Empty list #
(def b (list 1 2 3 4));     # List with values #
(def c "abc");              # String #
```

### Maps
Maps are generic key-value stores. Keys can be Strings, Numbers, or Booleans.
```lisp
(def f (map));                        # Empty map #
(def g (map "a" 1 "b" 2 99 "c"));     # Map with multiple key types #
```

### Indexing
The universal indexer `!!` works on Strings, Lists, and Maps. Note that there must be a space between the operator and the variable.
* **Lists/Strings:** Supports 0-based indexing and negative indexing (reverse order). Out-of-bounds indexing triggers an error.
* **Maps:** Returns the associated value. If the key does not exist, it safely returns an empty string `""`.

```lisp
(!! b 1)    # Returns the 2nd item of list b #
(!! b -1)   # Returns the last item of list b #
(!! g "a")  # Returns the value for key "a" in map g #
```

### Mutation and Allocation
* **`concat`**: Returns a *new* collection combining two existing collections.
    ```lisp
    (def d (concat b (list 5 6 7 8)));
    ```
* **`append`**: Mutates an existing list by pushing a value to the end.
    ```lisp
    (append b 5);
    ```
* **`pop`**: Mutates an existing list by removing and returning the final value.
    ```lisp
    (pop b);
    ```
* **`len`**: Returns the size of a list or string.
    ```lisp
    (len b);
    ```
* **`mset`**: Mutates a map by adding or updating a key-value pair.
    ```lisp
    (mset g "new_key" 100);
    ```
* **`mdel`**: Mutates a map by removing a specified key.
    ```lisp
    (mdel g "a");
    ```
* **`mkeys`**: Returns a list of all current keys in a map.
    ```lisp
    (mkeys g);
    ```



## 8. System Integration

### System Calls
The `sys` command evaluates its arguments and passes them directly to the underlying operating system kernel to be executed as shell commands.
```lisp
(sys echo 10);
(sys read "config.cfg");
(sys write "log.txt" "panic");
```

### Error Handling
The interpreter features a globally scoped `error` function. When `(error "message")` is called, it executes the current logic bound to the `error` variable, and then forcefully terminates the program with a `ProcessError` returned to the kernel shell. 

By default, the environment is initialized with:
```lisp
(def error (fn (s) (sys echo s)));
```
This behavior can be overwritten at runtime to route errors to logs or handle custom failure states before the program exits.
