use std::sync::Arc;

use once_cell::sync::OnceCell;
use savvy::{savvy, EnvironmentSexp};

use crate::environment::REnvironmentReader;
use crate::error::RGlareDbDatabaseError;
use crate::execution::RGlareDbExecutionOutput;
use crate::runtime::GLOBAL_RUNTIME;

#[savvy]
#[derive(Clone)]
struct RGlareDbConnection {
    pub(crate) inner: Arc<glaredb::Connection>,
}

impl RGlareDbConnection {
    // TODO: support async
    pub fn default_in_memory() -> savvy::Result<RGlareDbConnection> {
        static DEFAULT_CON: OnceCell<RGlareDbConnection> = OnceCell::new();

        let con = DEFAULT_CON.get_or_try_init(|| {
            GLOBAL_RUNTIME.0.block_on(async move {
                Ok(RGlareDbConnection {
                    inner: Arc::new(
                        glaredb::ConnectOptionsBuilder::new_in_memory()
                            .environment_reader(Arc::new(REnvironmentReader::new(
                                EnvironmentSexp::global_env(),
                            )))
                            .build()
                            .map_err(glaredb::DatabaseError::from)?
                            .connect()
                            .await?,
                    ),
                }) as Result<_, RGlareDbDatabaseError>
            })
        })?;

        Ok(con.clone())
    }

    pub fn sql(&self, query: &str) -> savvy::Result<RGlareDbExecutionOutput> {
        Ok(GLOBAL_RUNTIME
            .0
            .block_on(self.inner.sql(query).evaluate())
            .map_err(RGlareDbDatabaseError::from)?
            .into())
    }

    pub fn prql(&self, query: &str) -> savvy::Result<RGlareDbExecutionOutput> {
        Ok(GLOBAL_RUNTIME
            .0
            .block_on(self.inner.prql(query).evaluate())
            .map_err(RGlareDbDatabaseError::from)?
            .into())
    }

    pub fn execute(&self, query: &str) -> savvy::Result<RGlareDbExecutionOutput> {
        Ok(GLOBAL_RUNTIME
            .0
            .block_on(self.inner.execute(query).evaluate())
            .map_err(RGlareDbDatabaseError::from)?
            .into())
    }
}
