use crate::runtime::GLOBAL_RUNTIME;
use crate::table::RGlareDbTable;
use arrow::datatypes::Schema;
use async_trait::async_trait;
use datafusion::arrow::datatypes::SchemaRef;
use datafusion::datasource::TableProvider;
use datafusion::execution::context::SessionState;
use datafusion::execution::TaskContext;
use datafusion::logical_expr::{TableProviderFilterPushDown, TableType};
use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
use datafusion::physical_plan::streaming::{PartitionStream, StreamingTableExec};
use datafusion::physical_plan::ExecutionPlan;
use datafusion::prelude::Expr;
use glaredb::{DataFusionError, Operation, SendableRecordBatchStream};
use savvy::savvy;
use std::any::Any;
use std::sync::{Arc, Mutex};

#[savvy]
#[derive(Clone, Debug)]
pub struct RGlareDbExecutionOutput {
    op: Arc<Mutex<glaredb::Operation>>,
}

impl From<glaredb::Operation> for RGlareDbExecutionOutput {
    fn from(opt: glaredb::Operation) -> Self {
        Self {
            op: Arc::new(Mutex::new(opt)),
        }
    }
}

#[savvy]
impl RGlareDbExecutionOutput {
    fn print(&self) -> savvy::Result<()> {
        savvy::r_println!("RGlareDbExecution{:#?}", self.op);
        Ok(())
    }

    fn to_table(&self) -> savvy::Result<RGlareDbTable> {
        Ok(self.into())
    }
}

#[async_trait]
impl TableProvider for RGlareDbExecutionOutput {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        self.op
            .lock()
            .unwrap()
            .schema()
            .expect("table must be resolved before use")
    }

    fn table_type(&self) -> TableType {
        TableType::View
    }

    fn supports_filter_pushdown(
        &self,
        _filter: &Expr,
    ) -> Result<TableProviderFilterPushDown, DataFusionError> {
        Ok(TableProviderFilterPushDown::Inexact)
    }

    async fn scan(
        &self,
        _ctx: &SessionState,
        projection: Option<&Vec<usize>>,
        _filters: &[Expr],
        _limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>, DataFusionError> {
        let op = self.op.lock().unwrap();
        let schema = op.schema().expect("table must be resolved");
        Ok(Arc::new(StreamingTableExec::try_new(
            schema.clone(),
            vec![Arc::new(RPartition {
                schema: schema.clone(),
                exec: self.op.clone(),
            })],
            projection,
            None,
            false,
        )?))
    }
}

struct RPartition {
    schema: SchemaRef,
    exec: Arc<Mutex<Operation>>,
}

impl PartitionStream for RPartition {
    fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    fn execute(&self, _ctx: Arc<TaskContext>) -> SendableRecordBatchStream {
        let mut op = self.exec.lock().unwrap();

        Box::pin(RecordBatchStreamAdapter::new(
            self.schema.clone(),
            op.call(),
        ))
    }
}

impl From<&RGlareDbExecutionOutput> for RGlareDbTable {
    fn from(exec: &RGlareDbExecutionOutput) -> RGlareDbTable {
        let mut record_stream = exec.op.lock().unwrap().call();
        let batches = GLOBAL_RUNTIME
            .0
            .block_on(record_stream.to_vec())
            .expect("Must not fail"); // TODO: support async
        let schema = if batches.is_empty() {
            Arc::new(Schema::empty())
        } else {
            batches.first().unwrap().schema()
        };

        RGlareDbTable { schema, batches }
    }
}
