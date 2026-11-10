//! Port of `bandit.c` and the bandit algorithms `bandit_exp3.c`,
//! `bandit_exp3ix.c`, `bandit_epsgreedy.c` and `bandit_ucb.c`, which ALNS
//! uses to choose its neighborhoods. C's virtual function tables (plugins
//! without parameters) become the variants of [`BanditData`].

use crate::{
    misc::RandNumGen,
    retcode::{Retcode, ScipResult},
    scip::Scip,
};

/// Numerical tolerance of Exp.3.
const EXP3_NUMTOL: f64 = 1e-6;
/// Small number for tie breaking in epsilon greedy.
const EPSGREEDY_SMALL: f64 = 1e-6;
/// Numerical tolerance of UCB.
const UCB_NUMEPS: f64 = 1e-6;

/// `SCIP_BANDITDATA` of each algorithm.
#[derive(Clone, Debug)]
pub enum BanditData {
    /// Exp.3: exponential weights mixed with a uniform distribution.
    Exp3 { weights: Vec<f64>, weightsum: f64, gamma: f64, beta: f64 },
    /// Exp.3-IX: Exp.3 with implicit exploration.
    Exp3Ix { weights: Vec<f64>, weightsum: f64, iter: i32 },
    /// (Modified) epsilon greedy.
    EpsGreedy {
        weights: Vec<f64>,
        priorities: Vec<f64>,
        sels: Vec<i32>,
        eps: f64,
        usemodification: bool,
        decayfactor: f64,
        avglim: i32,
        nselections: i32,
        preferrecent: bool,
    },
    /// Upper confidence bounds.
    Ucb {
        nselections: i32,
        counter: Vec<i32>,
        startperm: Vec<i32>,
        meanscores: Vec<f64>,
        alpha: f64,
    },
}

/// `SCIP_BANDIT`.
#[derive(Clone, Debug)]
pub struct Bandit {
    rng: RandNumGen,
    nactions: i32,
    data: BanditData,
}

impl Bandit {
    /// `SCIPbanditCreate`: creates and resets a bandit algorithm.
    fn create(data: BanditData, priorities: Option<&[f64]>, nactions: i32, initseed: u32) -> ScipResult<Bandit> {
        // The number of actions must be positive.
        if nactions <= 0 {
            log::error!("Cannot create bandit selector with {nactions} <= 0 actions");
            return Err(Retcode::InvalidData);
        }

        let mut bandit = Bandit {
            rng: RandNumGen::new(initseed),
            nactions,
            data,
        };
        bandit.reset(priorities, initseed)?;
        Ok(bandit)
    }

    /// `SCIPbanditGetNActions`.
    pub fn get_n_actions(&self) -> i32 {
        self.nactions
    }

    /// `SCIPbanditGetRandnumgen`.
    pub fn rng(&mut self) -> &mut RandNumGen {
        &mut self.rng
    }

    /// `SCIPbanditGetData`.
    pub fn data(&self) -> &BanditData {
        &self.data
    }

    /// `SCIPbanditReset`: resets the bandit algorithm.
    pub fn reset(&mut self, priorities: Option<&[f64]>, seed: u32) -> ScipResult {
        // Test if the priorities are nonnegative.
        if let Some(priorities) = priorities {
            for (i, &p) in priorities[..self.nactions as usize].iter().enumerate() {
                if p < 0.0 {
                    log::error!("Negative priority for action {i}");
                    return Err(Retcode::InvalidData);
                }
            }
        }

        // Reset the random seed of the bandit algorithm.
        self.rng.set_seed(seed);

        // Call the reset callback of the bandit algorithm.
        let nactions = self.nactions as usize;
        let rng = &mut self.rng;
        match &mut self.data {
            BanditData::Exp3 { weights, weightsum, .. } => {
                // `SCIPbanditResetExp3`.
                *weightsum = (1.0 + EXP3_NUMTOL) * nactions as f64;
                weights.clear();
                weights.resize(nactions, 0.0);

                // In case of priorities, weights are normalized to sum up to
                // nactions.
                match priorities {
                    Some(priorities) => {
                        // Compute sum of priorities.
                        let mut priosum = 0.0;
                        for &p in &priorities[..nactions] {
                            priosum += p;
                        }

                        // If there are positive priorities, normalize the
                        // weights.
                        if priosum > 0.0 {
                            let normalization = nactions as f64 / priosum;
                            for i in 0..nactions {
                                weights[i] = (priorities[i] * normalization) + EXP3_NUMTOL;
                            }
                        } else {
                            // Use uniform distribution in case of all
                            // priorities being 0.0.
                            for w in weights.iter_mut() {
                                *w = 1.0 + EXP3_NUMTOL;
                            }
                        }
                    }
                    None => {
                        // Use uniform distribution in case of unspecified
                        // priorities.
                        for w in weights.iter_mut() {
                            *w = 1.0 + EXP3_NUMTOL;
                        }
                    }
                }
            }
            BanditData::Exp3Ix { weights, weightsum, iter } => {
                // `SCIPbanditResetExp3IX`: initialize all weights with 1.0.
                *weights = vec![1.0; nactions];
                *weightsum = nactions as f64;

                // Set iteration counter to 1.
                *iter = 1;
            }
            BanditData::EpsGreedy {
                weights,
                priorities: prios,
                sels,
                nselections,
                ..
            } => {
                // `SCIPbanditResetEpsgreedy`: alter priorities slightly to make
                // them unique.
                prios.resize(nactions, 0.0);
                match priorities {
                    Some(priorities) => {
                        // C leaves the first priority as allocated (not
                        // initialized); here it is zero.
                        for w in 1..nactions {
                            prios[w] = priorities[w] + rng.get_real(-EPSGREEDY_SMALL, EPSGREEDY_SMALL);
                        }
                    }
                    None => {
                        // Use random priorities.
                        for p in prios.iter_mut() {
                            *p = rng.get_real(0.0, 1.0);
                        }
                    }
                }

                // Reset weights and selection counters to 0.
                *weights = vec![0.0; nactions];
                *sels = vec![0; nactions];
                *nselections = 0;
            }
            BanditData::Ucb {
                nselections,
                counter,
                startperm,
                meanscores,
                ..
            } => {
                // `dataReset`: clear counters and scores.
                *counter = vec![0; nactions];
                *meanscores = vec![0.0; nactions];
                *nselections = 0;

                // Initialize start permutation as identity.
                *startperm = (0..nactions as i32).collect();

                // Prepare the start permutation in decreasing order of
                // priority.
                match priorities {
                    Some(priorities) => {
                        let mut prioritycopy = priorities[..nactions].to_vec();

                        // Randomly wiggle priorities a little bit to make them
                        // unique.
                        for p in prioritycopy.iter_mut() {
                            *p += rng.get_real(-UCB_NUMEPS, UCB_NUMEPS);
                        }

                        crate::sort::sort_down(&mut prioritycopy, (&mut startperm[..],), crate::sort::cmp_real);
                    }
                    None => {
                        // Use a random start permutation.
                        rng.permute(startperm, 0, nactions);
                    }
                }
            }
        }
        Ok(())
    }

    /// `SCIPbanditSelect`: selects the next action.
    pub fn select(&mut self) -> i32 {
        let nactions = self.nactions as usize;
        let rng = &mut self.rng;
        match &mut self.data {
            BanditData::Exp3 { weights, weightsum, gamma, .. } => {
                // Draw a random number between 0 and 1.
                let randnr = rng.get_real(0.0, 1.0);

                // Initialize some local variables to speed up probability
                // computations.
                let oneminusgamma = 1.0 - *gamma;
                let gammaoverk = *gamma / nactions as f64;

                // Loop over probability distribution until rand is reached;
                // the loop terminates in the last iteration.
                let mut psum = 0.0;
                let mut i = 0;
                while i < nactions - 1 {
                    // Compute the probability for arm i as convex combination
                    // of a uniform distribution and a weighted distribution.
                    let prob = oneminusgamma * weights[i] / *weightsum + gammaoverk;
                    psum += prob;

                    // Break and select element if target probability is
                    // reached.
                    if randnr <= psum {
                        break;
                    }
                    i += 1;
                }

                // Select element i, which is the last action in case that the
                // break statement hasn't been reached.
                i as i32
            }
            BanditData::Exp3Ix { weights, weightsum, .. } => {
                // Draw a random number between 0 and 1.
                let randnr = rng.get_real(0.0, 1.0);

                // Loop over probability distribution until rand is reached.
                let mut psum = 0.0;
                let mut i = 0;
                while i < nactions - 1 {
                    // Compute the probability for arm i.
                    let prob = weights[i] / *weightsum;
                    psum += prob;

                    // Break and select element if target probability is
                    // reached.
                    if randnr <= psum {
                        break;
                    }
                    i += 1;
                }

                // Select element i, which is the last action in case that the
                // break statement hasn't been reached.
                i as i32
            }
            BanditData::EpsGreedy {
                weights,
                priorities,
                eps,
                usemodification,
                nselections,
                ..
            } => {
                // Roll the dice to check if the best element should be picked,
                // or an element at random.
                let randnr = rng.get_real(0.0, 1.0);

                // Make epsilon decrease with an increasing number of
                // selections.
                *nselections += 1;
                let curreps = *eps * (nactions as f64 / *nselections as f64).sqrt();

                if randnr >= curreps {
                    // Select the best action seen so far: pick the element with
                    // the largest reward.
                    let mut maxweight = weights[0];
                    let mut selection = 0;

                    // Determine reward for every element.
                    for j in 1..nactions {
                        let weight = weights[j];

                        // Select the action that maximizes the reward, breaking
                        // ties by action priorities.
                        if maxweight < weight || (weight >= maxweight - EPSGREEDY_SMALL && priorities[j] > priorities[selection]) {
                            selection = j;
                            maxweight = weight;
                        }
                    }
                    selection as i32
                } else if !*usemodification {
                    // Use normal eps greedy: play one of the actions at random.
                    rng.get_int(0, nactions as i32 - 1)
                } else {
                    // Pick an action w.r.t. the distributions defined by its
                    // weights.
                    let mut weightsum = 0.0;
                    for &w in weights.iter() {
                        weightsum += w;
                    }

                    // Pick a random number between 0.0 and sum of weights.
                    let randnr = rng.get_real(0.0, weightsum);

                    // Pick action w.r.t. the weights distribution.
                    let mut sum = 0.0;
                    let mut selection = -1;
                    for (j, weight) in weights[..nactions - 1].iter().enumerate() {
                        sum += *weight;
                        if sum >= randnr {
                            selection = j as i32;
                            break;
                        }
                    }
                    if selection < 0 { nactions as i32 - 1 } else { selection }
                }
            }
            BanditData::Ucb {
                nselections,
                counter,
                startperm,
                meanscores,
                alpha,
            } => {
                if (*nselections as usize) < nactions {
                    // Select the next uninitialized action from the start
                    // permutation.
                    startperm[*nselections as usize]
                } else {
                    // Select the action with the highest upper confidence
                    // bound: compute the confidence width factor that is common
                    // for all actions.
                    let widthfactor = (*alpha * (*nselections as f64).ln_1p()).sqrt();
                    let mut maxucb = -1.0;
                    let mut selection = -1;

                    // Loop over the actions and determine the maximum upper
                    // confidence bound; the upper confidence bound of an action
                    // is the sum of its mean score and its confidence width.
                    for i in 0..nactions {
                        // Compute the upper confidence bound for action i.
                        let mut uppercb = meanscores[i];
                        let rootcount = (counter[i] as f64).sqrt();
                        uppercb += widthfactor / rootcount;

                        // Update maximum, breaking ties uniformly at random.
                        if uppercb - maxucb > UCB_NUMEPS || ((uppercb - maxucb).abs() <= UCB_NUMEPS && rng.get_real(0.0, 1.0) >= 0.5) {
                            maxucb = uppercb;
                            selection = i as i32;
                        }
                    }
                    selection
                }
            }
        }
    }

    /// `SCIPbanditUpdate`: updates the score of the selected action.
    pub fn update(&mut self, selection: i32, score: f64) {
        let nactions = self.nactions as usize;
        let s = selection as usize;
        match &mut self.data {
            BanditData::Exp3 { weights, weightsum, gamma, beta } => {
                // The learning rate eta.
                let eta = 1.0 / nactions as f64;
                let oneminusgamma = 1.0 - *gamma;
                let gammaoverk = *gamma * eta;
                let mut newweightsum = *weightsum;

                // If beta is zero, only the observation for the current arm
                // needs an update.
                if beta.abs() <= EXP3_NUMTOL {
                    let probai = oneminusgamma * weights[s] / *weightsum + gammaoverk;
                    let gainestim = score / probai;
                    newweightsum -= weights[s];
                    weights[s] *= (eta * gainestim).exp();
                    newweightsum += weights[s];
                } else {
                    newweightsum = 0.0;

                    // Loop over all items and update their weights based on the
                    // influence of the beta parameter.
                    for (j, weight) in weights[..nactions].iter_mut().enumerate() {
                        let probaj = oneminusgamma * *weight / *weightsum + gammaoverk;

                        // Consider the score only for the chosen arm i, use
                        // constant beta offset otherwise.
                        let gainestim = if j == s { (score + *beta) / probaj } else { *beta / probaj };
                        *weight *= (eta * gainestim).exp();
                        newweightsum += *weight;
                    }
                }

                *weightsum = newweightsum;
            }
            BanditData::Exp3Ix { weights, weightsum, iter } => {
                let mut newweightsum = *weightsum;

                // `SCIPcomputeGamma`.
                let gammaparam = ((nactions as f64).ln() / (4.0 * *iter as f64 * nactions as f64)).sqrt();
                let etaparam = 2.0 * gammaparam;

                // Probability of selection.
                let prob = weights[s] / *weightsum;

                // Estimated loss.
                let lossestim = (1.0 - score) / (prob + gammaparam);

                // Update the observation for the current arm.
                newweightsum -= weights[s];
                weights[s] *= (-etaparam * lossestim).exp();
                newweightsum += weights[s];

                *weightsum = newweightsum;

                // Increase iteration counter.
                *iter += 1;
            }
            BanditData::EpsGreedy {
                weights,
                sels,
                decayfactor,
                avglim,
                preferrecent,
                ..
            } => {
                // Increase the selection count.
                sels[s] += 1;

                // The very first observation is directly stored as weight for
                // both average or exponential decay.
                if sels[s] == 1 {
                    weights[s] = score;
                } else if *preferrecent && sels[s] > *avglim {
                    // Use exponentially decreasing weights for older
                    // observations: decrease old weights by decay factor.
                    weights[s] *= *decayfactor;
                    weights[s] += (1.0 - *decayfactor) * score;
                } else {
                    // Update average score.
                    let diff = score - weights[s];
                    weights[s] += diff / sels[s] as f64;
                }
            }
            BanditData::Ucb {
                nselections, counter, meanscores, ..
            } => {
                // Increase the mean by the incremental formula: A_n = A_n-1 +
                // 1/n (a_n - A_n-1).
                let delta = score - meanscores[s];
                counter[s] += 1;
                meanscores[s] += delta / counter[s] as f64;
                *nselections += 1;
            }
        }
    }

    /// `SCIPgetProbabilityExp3`.
    pub fn get_probability_exp3(&self, action: i32) -> f64 {
        match &self.data {
            BanditData::Exp3 { weights, weightsum, gamma, .. } => (1.0 - *gamma) * weights[action as usize] / *weightsum + *gamma / self.nactions as f64,
            _ => unreachable!(),
        }
    }

    /// `SCIPgetProbabilityExp3IX`.
    pub fn get_probability_exp3ix(&self, action: i32) -> f64 {
        match &self.data {
            BanditData::Exp3Ix { weights, weightsum, .. } => weights[action as usize] / *weightsum,
            _ => unreachable!(),
        }
    }

    /// `SCIPgetConfidenceBoundUcb`.
    pub fn get_confidence_bound_ucb(&self, action: i32) -> f64 {
        match &self.data {
            BanditData::Ucb {
                nselections,
                counter,
                meanscores,
                alpha,
                ..
            } => {
                // Since only scores between 0 and 1 are allowed, 1.0 is a sure
                // upper confidence bound.
                if *nselections < self.nactions {
                    return 1.0;
                }

                // The bandit algorithm must have picked every action once.
                meanscores[action as usize] + (*alpha * (*nselections as f64).ln_1p() / counter[action as usize] as f64).sqrt()
            }
            _ => unreachable!(),
        }
    }

    /// `SCIPgetWeightsEpsgreedy`.
    pub fn get_weights_epsgreedy(&self) -> &[f64] {
        match &self.data {
            BanditData::EpsGreedy { weights, .. } => weights,
            _ => unreachable!(),
        }
    }
}

/// `SCIPcreateBanditExp3`: creates and resets an Exp.3 bandit algorithm.
pub fn create_bandit_exp3(scip: &Scip, priorities: Option<&[f64]>, gammaparam: f64, beta: f64, nactions: i32, initseed: u32) -> ScipResult<Bandit> {
    let data = BanditData::Exp3 {
        weights: Vec::new(),
        weightsum: 0.0,
        gamma: gammaparam,
        beta,
    };
    Bandit::create(data, priorities, nactions, scip.set.initialize_random_seed(initseed))
}

/// `SCIPcreateBanditExp3IX`: creates and resets an Exp.3-IX bandit
/// algorithm.
pub fn create_bandit_exp3ix(scip: &Scip, priorities: Option<&[f64]>, nactions: i32, initseed: u32) -> ScipResult<Bandit> {
    let data = BanditData::Exp3Ix {
        weights: Vec::new(),
        weightsum: 0.0,
        iter: 1,
    };
    Bandit::create(data, priorities, nactions, scip.set.initialize_random_seed(initseed))
}

/// `SCIPcreateBanditEpsgreedy`: creates and resets an epsilon greedy
/// bandit algorithm.
#[allow(clippy::too_many_arguments)]
pub fn create_bandit_epsgreedy(
    scip: &Scip,
    priorities: Option<&[f64]>,
    eps: f64,
    usemodification: bool,
    preferrecent: bool,
    decayfactor: f64,
    avglim: i32,
    nactions: i32,
    initseed: u32,
) -> ScipResult<Bandit> {
    let data = BanditData::EpsGreedy {
        weights: Vec::new(),
        priorities: Vec::new(),
        sels: Vec::new(),
        eps,
        usemodification,
        decayfactor,
        avglim,
        nselections: 0,
        preferrecent,
    };
    Bandit::create(data, priorities, nactions, scip.set.initialize_random_seed(initseed))
}

/// `SCIPcreateBanditUcb`: creates and resets a UCB bandit algorithm.
pub fn create_bandit_ucb(scip: &Scip, priorities: Option<&[f64]>, alpha: f64, nactions: i32, initseed: u32) -> ScipResult<Bandit> {
    if alpha < 0.0 {
        log::error!("UCB requires nonnegative alpha parameter, have {alpha}");
        return Err(Retcode::InvalidData);
    }
    let data = BanditData::Ucb {
        nselections: 0,
        counter: Vec::new(),
        startperm: Vec::new(),
        meanscores: Vec::new(),
        alpha,
    };
    Bandit::create(data, priorities, nactions, scip.set.initialize_random_seed(initseed))
}
