create function add_user_to_class() returns TRIGGER as $$ begin insert into user_class (user_id, class_number, is_instructor) values (NEW.id, 'CSCI1001', false); RETURN NEW; end $$ LANGUAGE plpgsql
create trigger after_user_insert_test after insert on users for each row execute function add_user_to_class();
