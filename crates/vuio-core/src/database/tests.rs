use super::*;

fn assert_composed_manager<T>()
where
    T: DatabaseManager + MediaRepository + PlaylistRepository + HealthRepository + StatsRepository,
{
}

#[test]
fn redb_implements_every_repository_capability() {
    assert_composed_manager::<redb::RedbDatabase>();
}
