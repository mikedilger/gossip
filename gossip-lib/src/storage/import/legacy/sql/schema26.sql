insert into settings (key, value) values ('dark_mode', 1 - coalesce((SELECT value FROM settings WHERE key='light_mode'), 1));
delete from settings where key = 'light_mode';
