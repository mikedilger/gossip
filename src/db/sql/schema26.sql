insert into settings (key, value) values ('dark_mode', 1 - (SELECT value FROM settings WHERE key='light_mode'));
delete from settings where key = 'light_mode';
