use std::marker::PhantomData;
use std::os::raw::c_int;

use crate::error::Result;
use crate::ffi;
use crate::types::{Integer, LuaRef};
use crate::util::{
    assert_stack, geti, objlen, protect_lua, protect_lua_closure, rawlen, StackGuard,
};
use crate::value::{FromLua, Nil, ToLua, Value};

/// Handle to an internal Lua table.
#[derive(Clone, Debug)]
pub struct Table<'lua>(pub(crate) LuaRef<'lua>);

impl<'lua> Table<'lua> {
    /// Sets a key-value pair in the table.
    ///
    /// If the value is `nil`, this will effectively remove the pair.
    ///
    /// This might invoke the `__newindex` metamethod. Use the [`raw_set`] method if that is not
    /// desired.
    ///
    /// # Examples
    ///
    /// Export a value as a global to make it usable from Lua:
    ///
    /// ```
    /// # use rlua::{Lua, Result};
    /// # fn main() -> Result<()> {
    /// # Lua::new().context(|lua_context| {
    /// let globals = lua_context.globals();
    ///
    /// globals.set("assertions", cfg!(debug_assertions))?;
    ///
    /// lua_context.load(r#"
    ///     if assertions == true then
    ///         -- ...
    ///     elseif assertions == false then
    ///         -- ...
    ///     else
    ///         error("assertions neither on nor off?")
    ///     end
    /// "#).exec()?;
    /// # Ok(())
    /// # })
    /// # }
    /// ```
    ///
    /// [`raw_set`]: #method.raw_set
    pub fn set<K: ToLua<'lua>, V: ToLua<'lua>>(&self, key: K, value: V) -> Result<()> {
        let lua = self.0.lua;
        let key = key.to_lua(lua)?;
        let value = value.to_lua(lua)?;
        unsafe {
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 6);

            lua.push_ref(&self.0);
            lua.push_value(key)?;
            lua.push_value(value)?;

            unsafe extern "C" fn set_table(state: *mut ffi::lua_State) -> c_int {
                ffi::lua_settable(state, -3);
                1
            }
            protect_lua(lua.state, 3, set_table)
        }
    }

    /// Gets the value associated to `key` from the table.
    ///
    /// If no value is associated to `key`, returns the `nil` value.
    ///
    /// This might invoke the `__index` metamethod. Use the [`raw_get`] method if that is not
    /// desired.
    ///
    /// # Examples
    ///
    /// Query the version of the Lua interpreter:
    ///
    /// ```
    /// # use rlua::{Lua, Result};
    /// # fn main() -> Result<()> {
    /// # Lua::new().context(|lua_context| {
    /// let globals = lua_context.globals();
    ///
    /// let version: String = globals.get("_VERSION")?;
    /// println!("Lua version: {}", version);
    /// # Ok(())
    /// # })
    /// # }
    /// ```
    ///
    /// [`raw_get`]: #method.raw_get
    pub fn get<K: ToLua<'lua>, V: FromLua<'lua>>(&self, key: K) -> Result<V> {
        let lua = self.0.lua;
        let key = key.to_lua(lua)?;
        let value = unsafe {
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 5);

            lua.push_ref(&self.0);
            lua.push_value(key)?;

            unsafe extern "C" fn get_table(state: *mut ffi::lua_State) -> c_int {
                ffi::lua_gettable(state, -2);
                1
            }
            protect_lua(lua.state, 2, get_table)?;
            lua.pop_value()
        };
        V::from_lua(value, lua)
    }

    /// Checks whether the table contains a non-nil value for `key`.
    pub fn contains_key<K: ToLua<'lua>>(&self, key: K) -> Result<bool> {
        let lua = self.0.lua;
        let key = key.to_lua(lua)?;

        unsafe {
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 5);

            lua.push_ref(&self.0);
            lua.push_value(key)?;

            unsafe extern "C" fn get_table(state: *mut ffi::lua_State) -> c_int {
                ffi::lua_gettable(state, -2);
                1
            }
            protect_lua(lua.state, 2, get_table)?;

            let has = ffi::lua_isnil(lua.state, -1) == 0;
            Ok(has)
        }
    }

    /// Sets a key-value pair without invoking metamethods.
    pub fn raw_set<K: ToLua<'lua>, V: ToLua<'lua>>(&self, key: K, value: V) -> Result<()> {
        let lua = self.0.lua;
        let key = key.to_lua(lua)?;
        let value = value.to_lua(lua)?;

        unsafe {
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 6);

            lua.push_ref(&self.0);
            lua.push_value(key)?;
            lua.push_value(value)?;

            unsafe extern "C" fn raw_set(state: *mut ffi::lua_State) -> c_int {
                ffi::lua_rawset(state, -3);
                0
            }
            protect_lua(lua.state, 3, raw_set)?;

            Ok(())
        }
    }

    /// Gets the value associated to `key` without invoking metamethods.
    pub fn raw_get<K: ToLua<'lua>, V: FromLua<'lua>>(&self, key: K) -> Result<V> {
        let lua = self.0.lua;
        let key = key.to_lua(lua)?;
        let value = unsafe {
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 3);

            lua.push_ref(&self.0);
            lua.push_value(key)?;
            ffi::lua_rawget(lua.state, -2);
            lua.pop_value()
        };
        V::from_lua(value, lua)
    }

    /// Returns the result of the Lua `#` operator.
    ///
    /// This might invoke the `__len` metamethod. Use the [`raw_len`] method if that is not desired.
    ///
    /// [`raw_len`]: #method.raw_len
    pub fn len(&self) -> Result<Integer> {
        let lua = self.0.lua;
        unsafe {
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 4);
            lua.push_ref(&self.0);
            protect_lua_closure(lua.state, 1, 0, |state| objlen(state, -1))
        }
    }

    /// Returns the result of the Lua `#` operator, without invoking the `__len` metamethod.
    pub fn raw_len(&self) -> Integer {
        let lua = self.0.lua;
        unsafe {
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 1);
            lua.push_ref(&self.0);
            let len = rawlen(lua.state, -1);
            len as Integer
        }
    }

    /// Returns a reference to the metatable of this table, or `None` if no metatable is set.
    ///
    /// Unlike the `getmetatable` Lua function, this method ignores the `__metatable` field.
    pub fn get_metatable(&self) -> Option<Table<'lua>> {
        let lua = self.0.lua;
        unsafe {
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 1);
            lua.push_ref(&self.0);
            if ffi::lua_getmetatable(lua.state, -1) == 0 {
                None
            } else {
                let table = Table(lua.pop_ref());
                Some(table)
            }
        }
    }

    /// Sets or removes the metatable of this table.
    ///
    /// If `metatable` is `None`, the metatable is removed (if no metatable is set, this does
    /// nothing).
    pub fn set_metatable(&self, metatable: Option<Table<'lua>>) {
        let lua = self.0.lua;
        unsafe {
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 1);
            lua.push_ref(&self.0);
            if let Some(metatable) = metatable {
                lua.push_ref(&metatable.0);
            } else {
                ffi::lua_pushnil(lua.state);
            }
            ffi::lua_setmetatable(lua.state, -2);
        }
    }

    /// Consume this table and return an iterator over the pairs of the table.
    ///
    /// This works like the Lua `pairs` function, but does not invoke the `__pairs` metamethod.
    ///
    /// The pairs are wrapped in a [`Result`], since they are lazily converted to `K` and `V` types.
    ///
    /// # Note
    ///
    /// While this method consumes the `Table` object, it can not prevent code from mutating the
    /// table while the iteration is in progress. Refer to the [Lua manual] for information about
    /// the consequences of such mutation.
    ///
    /// # Examples
    ///
    /// Iterate over all globals:
    ///
    /// ```
    /// # use rlua::{Lua, Result, Value};
    /// # fn main() -> Result<()> {
    /// # Lua::new().context(|lua_context| {
    /// let globals = lua_context.globals();
    ///
    /// for pair in globals.pairs::<Value, Value>() {
    ///     let (key, value) = pair?;
    /// #   let _ = (key, value);   // used
    ///     // ...
    /// }
    /// # Ok(())
    /// # })
    /// # }
    /// ```
    ///
    /// [`Result`]: type.Result.html
    /// [Lua manual]: http://www.lua.org/manual/5.4/manual.html#pdf-next
    pub fn pairs<K: FromLua<'lua>, V: FromLua<'lua>>(self) -> TablePairs<'lua, K, V> {
        TablePairs {
            table: self.0,
            next_key: Some(Nil),
            _phantom: PhantomData,
        }
    }

    /// Consume this table and return an iterator over all values in the sequence part of the table.
    ///
    /// The iterator will yield all values `t[1]`, `t[2]`, and so on, until a `nil` value is
    /// encountered. This mirrors the behaviour of Lua's `ipairs` function and will invoke the
    /// `__index` metamethod according to the usual rules. However, the deprecated `__ipairs`
    /// metatable will not be called.
    ///
    /// Just like [`pairs`], the values are wrapped in a [`Result`].
    ///
    /// # Note
    ///
    /// While this method consumes the `Table` object, it can not prevent code from mutating the
    /// table while the iteration is in progress. Refer to the [Lua manual] for information about
    /// the consequences of such mutation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rlua::{Lua, Result, Table};
    /// # fn main() -> Result<()> {
    /// # Lua::new().context(|lua_context| {
    /// let my_table: Table = lua_context.load(r#"
    ///     {
    ///         [1] = 4,
    ///         [2] = 5,
    ///         [4] = 7,
    ///         key = 2
    ///     }
    /// "#).eval()?;
    ///
    /// let expected = [4, 5];
    /// for (&expected, got) in expected.iter().zip(my_table.sequence_values::<u32>()) {
    ///     assert_eq!(expected, got?);
    /// }
    /// # Ok(())
    /// # })
    /// # }
    /// ```
    ///
    /// [`pairs`]: #method.pairs
    /// [`Result`]: type.Result.html
    /// [Lua manual]: http://www.lua.org/manual/5.4/manual.html#pdf-next
    pub fn sequence_values<V: FromLua<'lua>>(self) -> TableSequence<'lua, V> {
        TableSequence {
            table: self.0,
            index: Some(1),
            _phantom: PhantomData,
        }
    }
}

/// An iterator over the pairs of a Lua table.
///
/// This struct is created by the [`Table::pairs`] method.
///
/// [`Table::pairs`]: struct.Table.html#method.pairs
pub struct TablePairs<'lua, K, V> {
    table: LuaRef<'lua>,
    next_key: Option<Value<'lua>>,
    _phantom: PhantomData<(K, V)>,
}

impl<'lua, K, V> Iterator for TablePairs<'lua, K, V>
where
    K: FromLua<'lua>,
    V: FromLua<'lua>,
{
    type Item = Result<(K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next_key) = self.next_key.take() {
            let lua = self.table.lua;

            let res = (|| {
                let res = unsafe {
                    let _sg = StackGuard::new(lua.state);
                    assert_stack(lua.state, 6);

                    lua.push_ref(&self.table);
                    lua.push_value(next_key)?;

                    if protect_lua_closure(lua.state, 2, ffi::LUA_MULTRET, |state| {
                        ffi::lua_next(state, -2) != 0
                    })? {
                        ffi::lua_pushvalue(lua.state, -2);
                        let key = lua.pop_value();
                        let value = lua.pop_value();
                        self.next_key = Some(lua.pop_value());

                        Some((key, value))
                    } else {
                        None
                    }
                };

                Ok(if let Some((key, value)) = res {
                    Some((K::from_lua(key, lua)?, V::from_lua(value, lua)?))
                } else {
                    None
                })
            })();

            match res {
                Ok(Some((key, value))) => Some(Ok((key, value))),
                Ok(None) => None,
                Err(e) => Some(Err(e)),
            }
        } else {
            None
        }
    }
}

/// An iterator over the sequence part of a Lua table.
///
/// This struct is created by the [`Table::sequence_values`] method.
///
/// [`Table::sequence_values`]: struct.Table.html#method.sequence_values
pub struct TableSequence<'lua, V> {
    table: LuaRef<'lua>,
    index: Option<Integer>,
    _phantom: PhantomData<V>,
}

impl<'lua, V> Iterator for TableSequence<'lua, V>
where
    V: FromLua<'lua>,
{
    type Item = Result<V>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(index) = self.index.take() {
            let lua = self.table.lua;

            let res = unsafe {
                let _sg = StackGuard::new(lua.state);
                assert_stack(lua.state, 5);

                lua.push_ref(&self.table);
                match protect_lua_closure(lua.state, 1, 1, |state| geti(state, -1, index)) {
                    Ok(ffi::LUA_TNIL) => None,
                    Ok(_) => {
                        let value = lua.pop_value();
                        self.index = Some(index + 1);
                        Some(Ok(value))
                    }
                    Err(err) => Some(Err(err)),
                }
            };

            match res {
                Some(Ok(r)) => Some(V::from_lua(r, lua)),
                Some(Err(err)) => Some(Err(err)),
                None => None,
            }
        } else {
            None
        }
    }
}
