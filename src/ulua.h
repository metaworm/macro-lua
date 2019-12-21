
#ifndef __ULUA_H__
#define __ULUA_H__

#define lua_lock(L) ulua_lock(L)
#define lua_unlock(L) ulua_unlock(L)
#define luai_userstateopen(L) ulua_init_lock(L)
// #define luai_userstatethread(L,L1) ulua_init_lock(L1)

extern void ulua_lock(lua_State * L);
extern void ulua_unlock(lua_State * L);
extern void ulua_init_lock(lua_State * L);

#endif /* __ULUA_H__ */
