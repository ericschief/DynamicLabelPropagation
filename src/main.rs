#![allow(non_snake_case)]
//extern crate blas_src;
use std::fs::File;
use std::default::Default;
use std::io::{BufRead,BufReader, Error};
//use std::time::Instant;
use smartcore::algorithm::neighbour::cover_tree::CoverTree;
use smartcore::math::distance::Distance;
use ndarray::Data;
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;

type FloatMat = Vec<Vec<f64>>;


#[derive(Debug)]

//these are parameters used in the algorithm
//k is for k-nearest neighbors and sigma is for calculating the affinity matrix of the dataset
//alpha and lambda are paramaters in the final steps of the algorithm, but the results are not sensitive to either of them
pub struct Params
{
    k: usize,
    sigma: f64,
    alpha: f64,
    lambda: f64,
    max_iter: usize,
}

impl Default for Params
{
    fn default() -> Self
    {
        return Params{k:30, sigma:0.06, alpha:0.05, lambda:0.1, max_iter:50};
    }
}

pub fn USPSlabels(file:&File)->Result<Vec<f64>,Error>
{
    let br = BufReader::new(file);
    //let n:usize = 7291; //fix this
    let mut labels = vec![];
    for line in br.lines()
    {
        let temp  = line?.trim().parse().unwrap();
        labels.push(temp);
    }
    Ok(labels)
}

pub fn USPSfeatures(file:&File) -> Result<FloatMat,Error>
{
    let mut features = vec![];
    let br = BufReader::new(file);
    for line in br.lines()
    {
        let mut row = vec![];
        for x in line?.trim().split(',')
        {
            let parsed:f64 = x.parse().unwrap();
            row.push(parsed);
        }
        features.push(row);
    }
    Ok(features)
}

pub fn norm<S>(x:&ArrayBase<S, Ix1>)->f64
where
    S:Data<Elem = f64>,
{
    return x.iter().fold(0.,|sum,x|sum + x*x).sqrt();
}

pub fn accuracyScore(vec1:&Vec<usize>,vec2:&Vec<usize>)
{
    let mut count:f64 = 0.;

    for i in 0..vec2.len()
    {
        if vec1[i] == vec2[i]
        {
            count += 1.;
        }

    }
    let accuracyScore:f64 = count/(vec1.len() as f64);
    println!("{}{}", accuracyScore, "\n");
}

#[derive(Clone)]
struct DistanceStruct<'a>
{
    graph: &'a Array2<f64>,
}

impl<'a> Distance<usize, f64> for DistanceStruct<'a>
{
    fn distance(&self, a:& usize, b:& usize) -> f64
    {
        self.graph[[*a,*b]]
    }
}

pub fn dist_graph(mat:&Array2<f64>) -> Array2<f64>
{
    let n = mat.shape()[0];
    let mut g = Array2::<f64>::zeros((n,n));//vec![vec![0.;n];n];
    for i in 0..n
    {
        for j in 0..n
        {
            let temp = &mat.row(i)-&mat.row(j);
            g[[i,j]] = norm(&temp);
        }
    }
    return g
}


//creates affinity matrix of a given graph
pub fn affinityMatrix(x:&Array2<f64>, params: &Params)->Array2<f64>
{
    let n = x.shape()[0];
    let mut w = Array2::<f64>::zeros((n,n));//vec![vec![0.;n]; n]; // allocate space for affinity matrix
    let mut val:f64;
    for i in 0..n
    {
        for j in 0..n
        {
            let dif = &x.row(i)-&x.row(j);// compute difference of two rows
            val = norm(&dif).powf(2.)/params.sigma;
            //find norm of vector
            w[[i,j]] = (-val).exp();

        }
    }
    return w
}

// calculates the similarity matrices that will be used to obtain the probabilistic transition matrices
pub fn calc_sim_mat(sampleMat:&Array2<f64>, params:&Params) -> (Array2<f64>,Array2<f64>)
{
    let num_samples = sampleMat.shape()[0];

    let mut ww = Array2::<f64>::zeros((num_samples,num_samples));//vec![vec![0.; num_samples]; num_samples];
    let affMat = affinityMatrix(&sampleMat,params);
    let g = dist_graph(&sampleMat);

    let ind:Vec<usize> =  (0..num_samples).collect();
    let tree = CoverTree::new(ind, DistanceStruct{graph: &g}).unwrap();

    for i in 0..num_samples
    {
        let knn = tree.find(&i, params.k).unwrap();
        for tup in knn
        {
            ww[[i,*tup.2]] = affMat[[i,*tup.2]];
        }
    }
    return (ww, affMat)
}

//creates probabilistic transition matrices for W and 𝓦
pub fn prob_trans_mat(sampleMat:&Array2<f64>,params:&Params)-> (Array2<f64>,Array2<f64>)
{
    let (w,ww) = calc_sim_mat(&sampleMat,&params);
    let n = w.shape()[0];
    let m = w.len_of(Axis(0));
    let mut p_0 = Array2::<f64>::zeros((n,m));//vec![vec![0.;m];n];
    let mut ps = Array2::<f64>::zeros((n,m));//vec![vec![0.;m];n];

    for i in 0..n
    {
        for j in 0..m
        {
            p_0[[i,j]] = w[[i,j]];// initialize matrix identical to x
            ps[[i,j]] = ww[[i,j]];
        }
    }

    for i in 0..n
    {
        let rowSum1:f64 = w.row(i).sum();
        let rowSum2:f64 = ww.row(i).sum();
        for j in 0..m
        {
            p_0[[i,j]] /= rowSum1; //sum row and divide each element in the row by that value
            ps[[i,j]] /= rowSum2;
        }
    }
    return (p_0,ps)
}
//lambda matrix used in final iteration steps of algorithm
//although lambda is a parameter, the algorithm is not very sensitive to changes in it
pub fn lamb_mat(dataSize:usize, params:&Params) -> Array2<f64>
{
    let n = 100;// This is the number of unlabeled samples
    let mut mat = Array2::<f64>::zeros((dataSize+n,dataSize+n));//vec![vec![0.;num_samples+n];num_samples+n];
    for i in 0..dataSize
    {
        mat[[i,i]] = params.lambda;
    }
    return mat
}

pub fn label_mat(numClasses:usize, labeledFeatures:&Array2<f64>, labels:&Vec<f64>,
    unlabeledFeatures:&Array2<f64>) -> Array2<f64>
{
    let numLabels = labeledFeatures.shape()[0] + unlabeledFeatures.shape()[0];

    let mut y = Array2::<f64>::zeros((numLabels,numClasses));
    for i in 0..labeledFeatures.shape()[0]
    {
        y[[i,labels[i] as usize]] = 1.;
    }
    return y
}

//dynamic label propagation needs training data and test data to work on
//sigma is a tuning parameter
pub fn dynamic_label_propagation(numClasses:usize, labeledFeatures:&Array2<f64>, labels:&Vec<f64>,
    unlabeledFeatures:&Array2<f64>, params:&Params)->(Array2<f64>,Vec<usize>)
{
    let dataSize = labeledFeatures.shape()[0];
    let y = label_mat(numClasses,&labeledFeatures, &labels, &unlabeledFeatures);

    let (mut p_0,ps) = prob_trans_mat(&labeledFeatures,params);

    let lambdaMat = lamb_mat(dataSize, params);

    let mut yNew = Array2::<f64>::zeros((p_0.shape()[0],y.shape()[1]));

    let psT = ps.t();

    for _i in 0..params.max_iter
    {
        yNew = p_0.dot(&y);

        for i in 0..dataSize
        {
            for j in 0..yNew.shape()[1]
            {
                yNew[[i,j]] = y[[i,j]];
            }
        }
        p_0 = &ps.dot(&(&p_0 + params.alpha*y.dot(&y.t()))).dot(&psT) + &lambdaMat;
        //p_0 = matSum(&matMult(&matMult(&ps,&matSum(&p_0,&scalarMult(params.alpha,matMult(&y,&y.t())))),&ps.t()),&lambdaMat);
    }

    let mut predictedLabels = vec![];

    for i in dataSize..yNew.shape()[0]
    {
        predictedLabels.push(yNew.row(i).argmax().unwrap());
    }

    return (yNew,predictedLabels)
}

fn main()
{


    let file1 = File::open("./TrainData/uspstrainlabels.txt").unwrap();
    let file2 = File::open("./TrainData/uspstrainfeatures.txt").unwrap();
    let file3 = File::open("./TestData/uspstestfeatures.txt").unwrap();


    let labelData = USPSlabels(&file1).unwrap();
    let featureData = USPSfeatures(&file2).unwrap();
    let testFeatures = USPSfeatures(&file3).unwrap();

    let numSamples = 200;


    let mut xTrain = Array2::<f64>::zeros((numSamples,256));
    let mut yTrain = vec![0.;numSamples];
    for i in 0..numSamples
    {
        for j in 0..256
        {
            xTrain[[i,j]] = featureData[i][j];
        }
        yTrain[i] = labelData[i];
    }

    let mut xTest = Array2::<f64>::zeros((100,256));
    for i in 0..100
    {
        for j in 0..256
        {
            xTest[[i,j]] = testFeatures[i][j];
        }
    }

    let classes:usize = 10;
    let _test = dynamic_label_propagation(classes,&xTrain,&yTrain,&xTest,&Default::default());



}
