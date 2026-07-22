use super::*;

impl GraphStore {
    pub(super) fn generate_regular(&mut self, degree: u32, seed: u64) -> Result<(), GraphError> {
        let n = self.node_count() as u32;
        if degree == 0 || degree >= n || (n * degree) % 2 != 0 {
            return Err(GraphError::RegularDegree { nodes: n, degree });
        }
        let mut labels = (0..n).collect::<Vec<_>>();
        for index in (1..labels.len()).rev() {
            let swap = deterministic_u64(seed, index as u64) as usize % (index + 1);
            labels.swap(index, swap);
        }
        let half = degree / 2;
        for position in 0..n {
            for offset in 1..=half {
                let other = (position + offset) % n;
                if self
                    .edge_between(labels[position as usize], labels[other as usize])
                    .is_none()
                {
                    self.add_edge(labels[position as usize], labels[other as usize])?;
                }
            }
        }
        if degree % 2 == 1 {
            if n % 2 != 0 {
                return Err(GraphError::RegularDegree { nodes: n, degree });
            }
            for position in 0..(n / 2) {
                self.add_edge(
                    labels[position as usize],
                    labels[(position + n / 2) as usize],
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn generate_scale_free(
        &mut self,
        links_per_node: u32,
        seed: u64,
    ) -> Result<(), GraphError> {
        let n = self.node_count() as u32;
        if n == 1 {
            return Ok(());
        }
        self.add_edge(0, 1)?;
        for node in 2..n {
            let target_count = links_per_node.min(node).max(1);
            let mut chosen = BTreeSet::new();
            let total_weight = (0..node)
                .map(|candidate| self.active_degree(candidate) as u64 + 1)
                .sum::<u64>();
            let mut draw_ordinal = 0_u64;
            while chosen.len() < target_count as usize {
                let mut draw =
                    deterministic_u64(seed ^ u64::from(node), draw_ordinal) % total_weight;
                draw_ordinal += 1;
                for candidate in 0..node {
                    let weight = self.active_degree(candidate) as u64 + 1;
                    if draw < weight {
                        chosen.insert(candidate);
                        break;
                    }
                    draw -= weight;
                }
            }
            for target in chosen {
                self.add_edge(node, target)?;
            }
        }
        Ok(())
    }
}
