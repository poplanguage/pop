# Pop concurrency example

[`concurrency.pop`](concurrency.pop) runs two structured child tasks over one
bounded `Channel<Int>`. The producer sends `10` and `20`, closes its sender,
the workers consume the values, and the parent awaits both children before
closing the receiver and returning `60`.

Check it with:

```text
pop check examples/concurrency.pop
```

The example uses only the typed `Task` and `Channel` APIs already present in
`Pop.Standard`; it does not use detached tasks or shared untyped state.
