# Implementation Shape

Primary ownership:

```text
src/entity/{mod,allocator}.rs
src/component/{mod,registry}.rs
src/storage/{mod,erased,sparse}.rs
src/math/{mod,q16}.rs
minimal final World construction/access path
```

Task order:

1. define opaque ids and allocator state transitions;
2. prove allocator against a simple randomized model;
3. define checked component policy/registry;
4. implement safe erased sparse storage;
5. connect final World registration/spawn/get/mutate/despawn path;
6. implement Q16 from the frozen half-away rounding/overflow rules;
7. add property, boundary, downstream API, allocation, and Divan cases.

Rejected operations must leave models unchanged. Tests force generation exhaustion and registration
conflicts; normal test volumes are not enough.
